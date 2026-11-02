import { createApp } from "vue";
import "@weppelmann/components/style.css";
import "@weppelmann/tokens-css";
import "@weppelmann/tokens-fonts";
import "./base.css";
import App from "./App.vue";
import { ladeEinstellungen } from "./store";

// Sprache und Farbschema kommen aus den Einstellungen — vor dem ersten Bild,
// sonst blitzt die Vorgabe auf.
ladeEinstellungen();

createApp(App).mount("#app");
