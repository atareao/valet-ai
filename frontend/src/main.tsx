import React from "react";
import ReactDOM from "react-dom/client";
import "./global.css";
import { LazyApp } from "./components/LazyApp";
import { registerServiceWorker } from "./push/registerServiceWorker";

registerServiceWorker();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <LazyApp />
  </React.StrictMode>,
);
