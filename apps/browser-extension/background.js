/**
 * PasteCanvas browser extension — MV3 background service worker (CAP-008).
 *
 * Creates context-menu capture actions and forwards them to the PasteCanvas
 * Native Messaging Host, which persists content into the local SQLite DB.
 * No cloud services are used — everything stays on the machine.
 */

// Must match the registered native messaging host name.
const NMH_NAME = "com.pastecanvas.capture";

// WIN-006: local IPC protocol version spoken to the native host.
const PROTOCOL_VERSION = 1;

/** Generates a short correlation id echoed back by the host (WIN-006). */
function newRequestId() {
  try {
    return crypto.randomUUID();
  } catch {
    return `req-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  }
}

// Create context menu entries on install.
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: "pc-page",
      title: "Save page to PasteCanvas",
      contexts: ["page"],
    });
    chrome.contextMenus.create({
      id: "pc-selection",
      title: "Save selection to PasteCanvas",
      contexts: ["selection"],
    });
    chrome.contextMenus.create({
      id: "pc-link",
      title: "Save link to PasteCanvas",
      contexts: ["link"],
    });
    chrome.contextMenus.create({
      id: "pc-image",
      title: "Save image to PasteCanvas",
      contexts: ["image"],
    });
    chrome.contextMenus.create({
      id: "pc-video",
      title: "Save video link to PasteCanvas",
      contexts: ["video", "audio"],
    });
  });
});

// Dispatch context menu clicks to the native host.
chrome.contextMenus.onClicked.addListener((info, tab) => {
  const request = buildRequest(info, tab);
  if (request) {
    sendToHost(request);
  }
});

/** Maps a context-menu click into a BrowserCaptureRequest. */
function buildRequest(info, tab) {
  switch (info.menuItemId) {
    case "pc-page":
      return {
        kind: "page",
        url: tab ? tab.url : info.pageUrl,
        title: tab ? tab.title : undefined,
      };
    case "pc-selection":
      return {
        kind: "selection",
        text: info.selectionText,
        sourceUrl: tab ? tab.url : info.pageUrl,
        title: tab ? tab.title : undefined,
      };
    case "pc-link":
      return {
        kind: "link",
        url: info.linkUrl,
        sourceUrl: tab ? tab.url : info.pageUrl,
        title: tab ? tab.title : undefined,
      };
    case "pc-image":
      return {
        kind: "image",
        url: info.srcUrl,
        sourceUrl: tab ? tab.url : info.pageUrl,
        title: tab ? tab.title : undefined,
      };
    case "pc-video":
      return {
        kind: "video",
        url: info.srcUrl,
        sourceUrl: tab ? tab.url : info.pageUrl,
        title: tab ? tab.title : undefined,
      };
    default:
      return null;
  }
}

/** Sends a capture request to the native messaging host. */
function sendToHost(request) {
  const requestId = newRequestId();
  const payload = { protocolVersion: PROTOCOL_VERSION, requestId, ...request };
  chrome.runtime.sendNativeMessage(NMH_NAME, payload, (response) => {
    if (chrome.runtime.lastError) {
      console.error("PasteCanvas NMH error:", chrome.runtime.lastError.message);
      return;
    }
    if (response && response.requestId !== requestId) {
      console.warn("PasteCanvas NMH: response/request id mismatch", response);
    }
    if (response && response.ok) {
      console.log("PasteCanvas captured item:", response.itemId);
    } else {
      console.error("PasteCanvas capture failed:", response && response.error);
    }
  });
}
