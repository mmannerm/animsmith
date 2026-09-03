//! Output streams a spawned CLI cannot deliver to.
//!
//! Several CLI contract tests run `animsmith` with a stdout whose reader is
//! gone, so its first write fails the way `animsmith lint … | head` does, and
//! then assert that the failure is diagnosed on stderr without rewriting the
//! command's own exit code.
//!
//! Building that pipe in the test process is not reliable while other tests
//! in the same binary are spawning children. In Rust 1.97's
//! `library/std/src/sys/pipe/unix.rs`, `std::io::pipe` creates the pair
//! atomically close-on-exec only where `pipe2` exists; on Apple targets — one
//! of the three this project's CI covers — it calls `pipe(2)` and *then* sets
//! `FD_CLOEXEC` on each end, and that toolchain's Apple spawn path
//! (`posix_spawn` without `POSIX_SPAWN_CLOEXEC_DEFAULT`) hands a child every
//! descriptor that is not yet marked. A child another test thread spawns
//! inside that window therefore inherits the read end and holds the pipe open
//! for its own lifetime; the CLI under test writes successfully into the pipe
//! buffer, no diagnosis is printed, and the assertion counting it fails. That
//! is the macOS-only flake in issue #690.
//!
//! [`ClosedStream`] removes the window rather than narrowing it: on Unix the
//! pipe is created between `fork` and `exec` in the child itself, so neither
//! end ever exists in a process that can pass it on. Windows keeps the plain
//! `std::io::pipe` form, where `CreatePipe` returns non-inheritable handles
//! and `CreateProcess` passes on only the handles a spawn marks inheritable.
//!
//! Closing the descriptor outright is not an alternative: Rust's runtime
//! reopens a missing standard descriptor as `/dev/null` at startup, and a
//! write to `/dev/null` succeeds. The write end has to stay open with nothing
//! reading it.

use std::process::{Command, Stdio};

/// Give a [`Command`] an output stream whose read end exists nowhere, so the
/// child's first write to it fails with a broken pipe.
///
/// No process other than the child can hold that read end on any of the three
/// platforms this project's CI covers; see the module documentation for how
/// each one gets there.
pub trait ClosedStream {
    /// Give the child a stdout that nothing is reading.
    fn closed_stdout(&mut self) -> &mut Self;

    /// Give the child a stderr that nothing is reading.
    fn closed_stderr(&mut self) -> &mut Self;
}

#[cfg(unix)]
impl ClosedStream for Command {
    fn closed_stdout(&mut self) -> &mut Self {
        self.stdout(Stdio::null());
        reader_less_pipe_on(self, libc::STDOUT_FILENO)
    }

    fn closed_stderr(&mut self) -> &mut Self {
        self.stderr(Stdio::null());
        reader_less_pipe_on(self, libc::STDERR_FILENO)
    }
}

/// Replace `target` in the spawned child with the write end of a pipe created
/// in that child, after `fork` and before `exec`.
#[cfg(unix)]
fn reader_less_pipe_on(command: &mut Command, target: libc::c_int) -> &mut Command {
    use std::os::unix::process::CommandExt;

    // SAFETY: the closure runs in the forked child, which is single-threaded
    // there, and before `exec`. Its only libc calls are `pipe`, `dup2` and
    // `close`, all async-signal-safe, and its only other calls are
    // `io::Error::last_os_error` and `Error::raw_os_error`, which read `errno`
    // into an inline representation and allocate nothing. Allocating after a
    // fork from this multi-threaded test binary is what would be unsound, and
    // this closure does not.
    unsafe {
        command.pre_exec(move || {
            let mut ends: [libc::c_int; 2] = [-1; 2];
            if libc::pipe(ends.as_mut_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let [reader, writer] = ends;
            loop {
                if libc::dup2(writer, target) >= 0 {
                    break;
                }
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::EINTR) {
                    close_pair(reader, writer, target);
                    return Err(error);
                }
            }
            // `target` is now a descriptor for the write end; dropping the
            // originals leaves it as the only one, and takes the read end
            // with them.
            close_pair(reader, writer, target);
            Ok(())
        });
    }
    command
}

/// Close a fresh pipe's own two descriptors, keeping `target` open.
///
/// `target` is normally neither of them, but this does not depend on that:
/// `pipe` returns the lowest free descriptors, so if `target` were closed
/// when it ran it could hand back `target` itself, and closing it here would
/// leave the child with no such stream at all rather than an unwritable one.
///
/// # Safety
///
/// Runs between `fork` and `exec`; `close` is async-signal-safe.
#[cfg(unix)]
unsafe fn close_pair(reader: libc::c_int, writer: libc::c_int, target: libc::c_int) {
    for end in [reader, writer] {
        if end != target {
            unsafe { libc::close(end) };
        }
    }
}

#[cfg(windows)]
impl ClosedStream for Command {
    fn closed_stdout(&mut self) -> &mut Self {
        self.stdout(reader_less_pipe())
    }

    fn closed_stderr(&mut self) -> &mut Self {
        self.stderr(reader_less_pipe())
    }
}

/// The write end of a pipe whose read end is already dropped.
#[cfg(windows)]
fn reader_less_pipe() -> Stdio {
    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    Stdio::from(writer)
}

/// These pin both helpers against silent degradation: a stream that quietly
/// became writable, and a pipe whose read end another process can reach.
///
/// They are Unix-only because the failure a closed stream produces there is
/// well defined (`SIGPIPE` or `EPIPE`) and because the leak they model is a
/// Unix descriptor leak. The Windows branch is a single function shared by
/// both `closed_stdout` and `closed_stderr`, and the CLI contract tests that
/// count the `cannot write … output to stdout` diagnostic run over it on the
/// Windows CI leg, so a degradation there fails those.
#[cfg(all(test, unix))]
mod tests {
    use super::ClosedStream;
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::process::{Child, Command, Stdio};

    /// A shell that writes one line to the stream under test.
    fn writes_to(stream: &str) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(match stream {
            "stdout" => "echo x",
            _ => "echo x >&2",
        });
        command
    }

    /// The write end of a pipe whose read end a live child inherited.
    ///
    /// The pipe is deliberately created without `FD_CLOEXEC`, which is the
    /// state `std::io::pipe` leaves an Apple-target pair in between its
    /// `pipe(2)` and its `set_cloexec`. A child spawned in that state
    /// inherits both ends and holds them for its own lifetime.
    struct LeakedPipe {
        holder: Child,
        writer: OwnedFd,
    }

    impl LeakedPipe {
        fn create() -> Self {
            let mut ends: [libc::c_int; 2] = [-1; 2];
            // SAFETY: `ends` is a valid two-element array for `pipe` to fill.
            assert_eq!(
                unsafe { libc::pipe(ends.as_mut_ptr()) },
                0,
                "creates a pipe"
            );
            let [reader, writer] = ends;

            let holder = Command::new("sleep")
                .arg("30")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawns the descriptor-hoarding child");

            // The parent drops its own read end, exactly as a test that built
            // this stream itself would. The holder still has one.
            // SAFETY: `reader` is this process's own descriptor, closed once.
            assert_eq!(unsafe { libc::close(reader) }, 0, "drops the read end");

            Self {
                holder,
                // SAFETY: `writer` is a fresh descriptor this owns from here.
                writer: unsafe { OwnedFd::from_raw_fd(writer) },
            }
        }

        fn release(mut self) {
            self.holder.kill().expect("kills the holder");
            self.holder.wait().expect("reaps the holder");
        }
    }

    #[test]
    fn a_readable_stdout_lets_the_child_write() {
        let output = writes_to("stdout")
            .stdout(Stdio::piped())
            .output()
            .expect("runs the writer");
        assert!(output.status.success(), "status: {}", output.status);
        assert_eq!(output.stdout, b"x\n");
    }

    #[test]
    fn a_closed_stdout_fails_the_child_write() {
        let status = writes_to("stdout")
            .closed_stdout()
            .stderr(Stdio::null())
            .status()
            .expect("runs the writer");
        assert!(
            !status.success(),
            "a write into a reader-less stdout must fail; status: {status}"
        );
    }

    #[test]
    fn a_closed_stderr_fails_the_child_write() {
        let status = writes_to("stderr")
            .closed_stderr()
            .stdout(Stdio::null())
            .status()
            .expect("runs the writer");
        assert!(
            !status.success(),
            "a write into a reader-less stderr must fail; status: {status}"
        );
    }

    /// The cause of issue #690, as an executable statement: a pipe whose read
    /// end any other process still holds accepts the write. This is why the
    /// stream cannot be built in the parent, where a concurrent spawn can
    /// inherit the read end before it is marked close-on-exec.
    #[test]
    fn a_reader_another_process_holds_keeps_the_pipe_writable() {
        let leaked = LeakedPipe::create();
        let status = writes_to("stdout")
            .stdout(Stdio::from(leaked.writer.try_clone().expect("clones")))
            .stderr(Stdio::null())
            .status()
            .expect("runs the writer");
        leaked.release();
        assert!(
            status.success(),
            "the holder's read end keeps this write succeeding; status: {status}"
        );
    }

    /// The property [`ClosedStream`] buys: the same descriptor-hoarding child
    /// is alive, and the write still fails, because the pipe is created inside
    /// the writing child and no other process can name either end.
    #[test]
    fn a_child_built_stdout_fails_beside_a_descriptor_hoarding_child() {
        let leaked = LeakedPipe::create();
        let status = writes_to("stdout")
            .closed_stdout()
            .stderr(Stdio::null())
            .status()
            .expect("runs the writer");
        leaked.release();
        assert!(
            !status.success(),
            "no other process can hold this pipe's read end; status: {status}"
        );
    }
}
