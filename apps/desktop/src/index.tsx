/* @refresh reload */
import { render } from "solid-js/web";
import App from "./App";

window.addEventListener("error", (e) => {
  console.error("UNCAUGHT JS ERROR:", e.error ?? e.message, e);
});
window.addEventListener("unhandledrejection", (e) => {
  console.error("UNHANDLED PROMISE REJECTION:", e.reason);
});

render(() => <App />, document.getElementById("root")!);
