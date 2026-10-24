import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app/App";
import { bootstrapTheme } from "./app/bootstrap";
import "./styles/global.css";

// Before the first render, so the window never paints the default theme and then
// repaints into the stored one.
bootstrapTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
