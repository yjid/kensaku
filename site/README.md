# Kensaku download page

From the repository root, run `python3 -m http.server 8000 --bind 127.0.0.1 --directory site`
and open <http://127.0.0.1:8000/>. The page fetches the latest stable release
from GitHub and caches its version and asset names for six hours in the browser.
The HTML links go to the latest release, so they work with JavaScript disabled
and require no updates when a new version ships.

## First deployment

1. In the upstream repository, set **Settings → Pages → Source** to **GitHub Actions**.
2. Once the workflow is on `master`, open **Actions → Download site → Run workflow**
   and choose `master`. Later changes under `site/` or to the Pages workflow
   also trigger deployment on `master`.
3. After the workflow succeeds, open <https://yjid.github.io/kensaku/> while
   signed out. Check that both screenshots load, the installer and checksum
   links resolve, and disabling JavaScript leaves usable links to GitHub Releases.
4. After those checks, add the site to the project README and release notes.
