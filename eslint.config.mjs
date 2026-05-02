import appConfig from "./ob-poc-ui-react/eslint.config.js";

export default [
  {
    ignores: [
      "artifacts/**",
      "observatory-wasm/pkg/**",
      "observatory-wasm/target/**",
      "static/pkg/**",
      "**/node_modules/**",
      "**/dist/**",
    ],
  },
  ...appConfig,
];
