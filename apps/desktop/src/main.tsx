import React from "react";
import ReactDOM from "react-dom/client";
import { currentWindowLabel } from "./lib/runtime";
import { App } from "./App";
import { RegionSelect } from "./features/capture/RegionSelect";
import { QuickCapture } from "./features/capture/QuickCapture";
import "./styles.css";

// Each Tauri window is its own surface. The quick-capture label must not render
// the full workspace shell: doing so made the supposedly small floating window
// show the entire App (and could steal focus from the source application).
const windowLabel = currentWindowLabel();
const Root = windowLabel === "region-overlay"
  ? RegionSelect
  : windowLabel === "quick-capture"
    ? QuickCapture
    : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
