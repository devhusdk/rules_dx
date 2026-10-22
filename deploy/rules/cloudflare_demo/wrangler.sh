#!/bin/sh
project=""
branch=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --project-name) project="$2"; shift ;;
    --branch) branch="$2"; shift ;;
  esac
  shift
done
if [ "$CLOUDFLARE_API_TOKEN" = "bad-token" ]; then
  echo "Authentication error [code: 10000] token $CLOUDFLARE_API_TOKEN" >&2
  exit 9
fi
case "$branch" in
  main) environment=production ;;
  *) environment=preview ;;
esac
slug=$(printf '%s' "$branch" | tr '/._' '---')
printf '{"type":"pages-deploy-detailed","version":1,"pages_project":"%s","deployment_id":"dep-%s","url":"https://%s.%s.pages.dev","environment":"%s"}\n' \
  "$project" "${AMBIENT_SECRET:-clean}" "$slug" "$project" "$environment" > "$WRANGLER_OUTPUT_FILE_PATH"
