import React from "react";
import ReactDOM from "react-dom/client";
import "./global.css";
import App from "./App";
import { registerServiceWorker } from "./push/registerServiceWorker";

registerServiceWorker();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
