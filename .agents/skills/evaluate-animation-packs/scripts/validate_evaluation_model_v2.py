#!/usr/bin/env python3
"""Validate evaluation model V2 against exact collection-output V11."""

from collection_evaluation import CollectionEvaluationValidator

validator = CollectionEvaluationValidator(2)
validate_model = validator.validate_model
validate_with_animsmith = validator.validate_with_animsmith
load_authoritative_collection_output = validator.load_authoritative_collection_output
main = validator.main

if __name__ == "__main__":
    raise SystemExit(main())
