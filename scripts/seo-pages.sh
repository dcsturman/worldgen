#!/bin/sh
# Turn Trunk's single dist/index.html into one crawlable page per tool, and
# write robots.txt and sitemap.xml. See seo/README.md. Run after `trunk build`:
#   ./scripts/seo-pages.sh dist
set -eu

DIST=${1:-dist}
SEO=$(dirname "$0")/../seo
SITE=https://tools.callistoflight.com
PAGES="index trade world worldmap simulator pirate-simulator"
SHELL_HTML="$DIST/.shell.html"

# Fail loudly if Trunk stops passing the markers through: a page without them
# would ship as the old blank shell and nobody would notice.
for marker in '<!-- seo:head -->' '<!-- seo:about -->'; do
    grep -q "$marker" "$DIST/index.html" || { echo "seo-pages: $marker missing from $DIST/index.html" >&2; exit 1; }
done
cp "$DIST/index.html" "$SHELL_HTML"

for page in $PAGES; do
    head="$SEO/$page.head.html"
    body="$SEO/$page.body.html"
    [ -f "$head" ] && [ -f "$body" ] || { echo "seo-pages: missing $head or $body" >&2; exit 1; }
    if [ "$page" = index ]; then
        out="$DIST/index.html"
    else
        # dist/trade.html, which nginx serves for /trade (try_files
        # $uri.html). A trade/index.html would be served only after a 301 to
        # /trade/, away from the canonical URL every link uses.
        out="$DIST/$page.html"
    fi
    # Drop the shell's placeholder <title>, and splice each file in at its marker.
    awk -v head="$head" -v body="$body" '
        /<title>/ { next }
        index($0, "<!-- seo:head -->")  { while ((getline l < head) > 0) print l; close(head); next }
        index($0, "<!-- seo:about -->") { while ((getline l < body) > 0) print l; close(body); next }
        { print }
    ' "$SHELL_HTML" > "$out"
done
rm "$SHELL_HTML"

{
    echo '<?xml version="1.0" encoding="UTF-8"?>'
    echo '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">'
    for page in $PAGES; do
        if [ "$page" = index ]; then path=/; else path=/$page; fi
        echo "  <url><loc>$SITE$path</loc></url>"
    done
    echo '</urlset>'
} > "$DIST/sitemap.xml"

printf 'User-agent: *\nAllow: /\nDisallow: /api/\n\nSitemap: %s/sitemap.xml\n' "$SITE" > "$DIST/robots.txt"
echo "seo-pages: wrote $(echo $PAGES | wc -w | tr -d ' ') pages, sitemap.xml and robots.txt into $DIST"
