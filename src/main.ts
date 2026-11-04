import { mount } from "svelte";
// Only downloaded by the browser when a theme uses it.
import "@fontsource-variable/space-grotesk";
import "./app.css";
import App from "./App.svelte";
import WidgetPanel from "./lib/WidgetPanel.svelte";

// The widget panel is the same app in its own small window, opened at #widgets.
const widgets = location.hash === "#widgets";
if (widgets) document.documentElement.classList.add("widget-mode");

export default mount(widgets ? WidgetPanel : App, { target: document.getElementById("app")! });
