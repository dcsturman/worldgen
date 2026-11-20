# Per-page SEO text

The app is one WASM bundle behind one `index.html`, so without these every URL
served the same blank page titled "Worldgen", and crawlers indexed none of the
tools. `scripts/seo-pages.sh` (run after `trunk build` in the Dockerfile) turns
`dist/index.html` into one page per tool:

- `<page>.head.html` replaces the `<!-- seo:head -->` marker (title,
  description, canonical, Open Graph).
- `<page>.body.html` replaces the `<!-- seo:about -->` marker: plain text about
  the tool, shown under the app (the app moves it to the end of the page once it
  mounts), so it's the same for a crawler, a no-JS visitor and a user.

`index` becomes `dist/index.html`; every other page `dist/<page>.html`, which
nginx serves for `/<page>` (`try_files $uri $uri.html …`). Add a page by adding both files and listing it
in `PAGES` in the script and in `sitemap.xml` (written by the script).
