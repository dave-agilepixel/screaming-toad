import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@fontsource/dm-sans/400";
import "@fontsource/dm-sans/500";
import "@fontsource/dm-sans/600";
import "@fontsource/dm-sans/700";
import "@fontsource/big-shoulders-display/800";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
