"use strict";

// Progressive enhancement only: every link works without this file.
const REPO = "yjid/kensaku";
const RELEASES = `https://github.com/${REPO}/releases`;
const LATEST_API = `https://api.github.com/repos/${REPO}/releases/latest`;

const LABELS = {
  windows: "Download for Windows",
  macos: "Download for macOS",
  linux: "Download for Linux",
};

function detectPlatform() {
  const ua = navigator.userAgent;
  const platform = navigator.userAgentData?.platform || ua;
  if (navigator.userAgentData?.mobile || /Android|iPhone|iPad|iPod|Mobile/i.test(ua)) return null;
  if (/Windows/i.test(platform)) return "windows";
  if (/macOS|Macintosh/i.test(platform)) return "macos";
  if (/Linux|X11/i.test(platform)) return "linux";
  return null;
}

function markDetected(name) {
  if (!name) return;
  const card = document.querySelector(`[data-platform="${name}"]`);
  if (card) {
    card.classList.add("recommended");
    const tag = card.querySelector(".tag");
    if (tag) tag.hidden = false;
  }
  const primary = document.querySelector("[data-primary]");
  if (primary) {
    primary.textContent = LABELS[name];
    primary.dataset.platform = name;
    syncPrimary();
  }
}

// The hero button always points at the first installer of the detected system.
function syncPrimary() {
  const primary = document.querySelector("[data-primary]");
  const name = primary?.dataset.platform;
  const first = name && document.querySelector(`[data-platform="${name}"] [data-asset]`);
  if (first) primary.href = first.href;
}

function setLinkToReleasePage(link) {
  link.href = `${RELEASES}/latest`;
  link.removeAttribute("aria-label");
}

async function refreshFromLatestRelease() {
  const status = document.querySelector("[data-release-status]");
  const links = document.querySelectorAll("[data-asset]");
  try {
    const response = await fetch(LATEST_API, {
      credentials: "omit",
      referrerPolicy: "no-referrer",
      signal: AbortSignal.timeout?.(5000),
    });
    if (!response.ok) throw new Error("lookup failed");
    const release = await response.json();
    const tag = release.tag_name;
    if (!/^v\d+\.\d+\.\d+$/.test(tag) || !Array.isArray(release.assets)) throw new Error("bad metadata");

    const names = new Set(release.assets.map((a) => a?.name));
    const version = tag.slice(1);
    for (const link of links) {
      const file = link.dataset.asset.replaceAll("{version}", version);
      if (names.has(file)) {
        link.href = `${RELEASES}/download/${tag}/${file}`;
      } else {
        setLinkToReleasePage(link);
      }
    }
    syncPrimary();
    if (status) status.textContent = `Version ${version}`;
  } catch {
    // Keep the built-in links. If they point at an old version, send people to the release list.
    if (status) status.textContent = "Latest release";
  }
}

const detected = detectPlatform();
markDetected(detected);
refreshFromLatestRelease();
