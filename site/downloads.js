"use strict";

const releases = "https://github.com/yjid/kensaku/releases";
const endpoint = "https://api.github.com/repos/yjid/kensaku/releases/latest";
const cacheKey = "kensaku:latest-release";
const cacheLifetime = 6 * 60 * 60 * 1000;

function highlightPlatform() {
  const agent = navigator.userAgent;
  const platform = navigator.userAgentData?.platform || agent;

  if (navigator.userAgentData?.mobile ||
      /Android|iPhone|iPad|iPod|Windows Phone|CrOS|Mobile/i.test(agent) ||
      (/Macintosh/i.test(agent) && navigator.maxTouchPoints > 1)) {
    return;
  }

  let name;
  if (/Windows/i.test(platform)) name = "windows";
  else if (/macOS|Macintosh/i.test(platform)) name = "macos";
  else if (/Linux/i.test(platform)) name = "linux";

  if (name) {
    const card = document.querySelector(`[data-platform="${name}"]`);
    card.classList.add("recommended");
    card.querySelector(".platform-badge").hidden = false;
  }
}

function validRelease(release) {
  return release &&
    typeof release.tag === "string" &&
    /^v\d+\.\d+\.\d+$/.test(release.tag) &&
    Array.isArray(release.filenames) &&
    release.filenames.every((name) => typeof name === "string");
}

function readCachedRelease() {
  try {
    const release = JSON.parse(localStorage.getItem(cacheKey));
    if (!validRelease(release) || !Number.isFinite(release.fetchedAt)) return null;
    const age = Date.now() - release.fetchedAt;
    return age >= 0 && age < cacheLifetime ? release : null;
  } catch {
    return null;
  }
}

async function fetchRelease() {
  const response = await fetch(endpoint, {
    credentials: "omit",
    referrerPolicy: "no-referrer",
    signal: globalThis.AbortSignal?.timeout?.(4500),
  });
  if (!response.ok) return null;

  const data = await response.json();
  if (!Array.isArray(data?.assets) || data.draft || data.prerelease) return null;
  const release = {
    tag: data.tag_name,
    filenames: data.assets.map((asset) => asset?.name),
    fetchedAt: Date.now(),
  };
  if (!validRelease(release)) return null;

  try {
    localStorage.setItem(cacheKey, JSON.stringify(release));
  } catch {
    // The current visit can still use the fetched release when storage is blocked.
  }
  return release;
}

function installerName(template, version, filenames) {
  if (template === "rpm") {
    const rpmPrefix = `kensaku-${version}-`;
    return filenames.find((name) => name.startsWith(rpmPrefix) &&
      /^\d+\.x86_64\.rpm$/.test(name.slice(rpmPrefix.length)));
  }
  const filename = template.replace("{version}", version);
  return filenames.includes(filename) ? filename : undefined;
}

async function updateDownloads() {
  // The HTML already points to GitHub Releases if the lookup is unavailable.
  const release = readCachedRelease() ?? await fetchRelease().catch(() => null);
  if (!release) return;

  const downloadBase = `${releases}/download/${release.tag}`;
  const version = release.tag.slice(1);
  for (const link of document.querySelectorAll("[data-asset]")) {
    const filename = installerName(link.dataset.asset, version, release.filenames);
    if (!filename) continue;

    link.href = `${downloadBase}/${filename}`;
    link.querySelector("[data-release-label]").hidden = true;
    if (link.dataset.installerLabel) {
      link.setAttribute("aria-label", link.dataset.installerLabel);
      link.querySelector("[data-link-arrow]").textContent = "↓";
    }
  }

  const checksums = document.querySelector("[data-release-checksums]");
  if (release.filenames.includes("SHA256SUMS")) {
    checksums.href = `${downloadBase}/SHA256SUMS`;
  } else {
    checksums.href = `${releases}/tag/${release.tag}`;
    checksums.textContent = "Release details (checksums unavailable)";
  }
  document.querySelector("[data-release-status]").textContent = `Release ${release.tag}`;
}

highlightPlatform();
updateDownloads();
