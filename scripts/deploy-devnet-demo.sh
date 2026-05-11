#!/usr/bin/env bash
# Deploy static Guardian demo (NVIDIA gate) to Cloudflare Pages project sak-devnet-test.
# Prereq: wrangler login. Override: CF_PAGES_DEMO_PROJECT=my-name npx ...
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="${CF_PAGES_DEMO_PROJECT:-sak-devnet-test}"
bash "${ROOT}/scripts/bundle-static-demo.sh"
echo "Deploying to Cloudflare Pages project: ${PROJECT}"
npx wrangler pages deploy "${ROOT}/.pages-out" --project-name="${PROJECT}" --commit-dirty=true
