import React from "react";
import ReactDOM from "react-dom/client";

import { bootstrapTheme } from "../app/bootstrap";
import PanelScreen from "./PanelScreen";
import "../styles/global.css";

// The same bootstrap the dashboard uses, so one stored preference cannot leave
// the two windows showing different themes.
bootstrapTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <PanelScreen />
  </React.StrictMode>,
);