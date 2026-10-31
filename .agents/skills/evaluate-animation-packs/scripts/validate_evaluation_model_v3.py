#!/usr/bin/env python3
"""Validate evaluation model V3 against exact collection-output V12."""

from collection_evaluation import CollectionEvaluationValidator

validator = CollectionEvaluationValidator(3)
validate_model = validator.validate_model
validate_with_animsmith = validator.validate_with_animsmith
load_authoritative_collection_output = validator.load_authoritative_collection_output
main = validator.main

if __name__ == "__main__":
    raise SystemExit(main())
