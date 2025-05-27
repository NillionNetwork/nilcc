"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var vite_tsconfig_paths_1 = require("vite-tsconfig-paths");
var config_1 = require("vitest/config");
exports.default = (0, config_1.defineConfig)({
    plugins: [(0, vite_tsconfig_paths_1.default)()],
    test: {
        globalSetup: "./tests/fixture/global-setup.ts",
        testTimeout: 0,
        env: {
            DEBUG: "@nillion*",
        },
        coverage: {
            reporter: ["text", "json-summary", "json"],
            reportOnFailure: true,
        },
    },
});
