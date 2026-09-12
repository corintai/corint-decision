import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { initializeTheme } from "./components/ThemeControl";
import "@xyflow/react/dist/style.css";
import "./theme.css";
import "./styles.css";

initializeTheme();
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
