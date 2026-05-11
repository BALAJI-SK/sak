#!/usr/bin/env bash
# Builds a minimal folder for static hosting (Cloudflare Pages, Netlify, etc.):
#   .pages-out/index.html  +  .pages-out/fonts/*
#
# Deploy to Cloudflare Pages (install Wrangler once: npm i -g wrangler, then wrangler login):
#   npx wrangler pages deploy .pages-out --project-name="${CF_PAGES_PROJECT:-sak}"
#
# After deploy, open the *.pages.dev URL (not github.io) unless you also updated GitHub Pages.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${ROOT}/.pages-out"
rm -rf "${OUT}"
mkdir -p "${OUT}/fonts"
cp "${ROOT}/index.html" "${OUT}/"
if [[ -d "${ROOT}/fonts" ]]; then
  cp -R "${ROOT}/fonts/." "${OUT}/fonts/"
fi
if [[ -d "${ROOT}/demo/assets" ]]; then
  mkdir -p "${OUT}/assets"
  cp -R "${ROOT}/demo/assets/." "${OUT}/assets/"
fi
echo "Static bundle ready: ${OUT}"
echo "Cloudflare: npx wrangler pages deploy \"${OUT}\" --project-name=\"\${CF_PAGES_PROJECT:-sak}\""
