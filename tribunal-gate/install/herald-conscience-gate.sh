#!/usr/bin/env bash
# Herald-conscience publish hook — gate the publish with tribunal-gate.
# Invoked from herald-pack/herald-market before writing marketplace entry.
set -uo pipefail
tribunal-gate --owl "${CONSCIENCE_OWL:-world-ontology.owl}" \
              --guard "${CONSCIENCE_GUARD:-ousia-guard}" \
              --corpus "${CONSCIENCE_CORPUS:-corpus/}" \
              "$@"
