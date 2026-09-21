import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import fs from "fs";

function hashPathResolverPlugin() {
  const projectRoot = __dirname;

  function resolveWithExt(filePath: string): string | null {
    if (fs.existsSync(filePath)) {
      const stat = fs.statSync(filePath);
      if (stat.isFile()) return filePath;
      if (stat.isDirectory()) {
        for (const ext of [".tsx", ".ts", ".jsx", ".js"]) {
          const indexFile = path.join(filePath, "index" + ext);
          if (fs.existsSync(indexFile) && fs.statSync(indexFile).isFile()) {
            return indexFile;
          }
        }
      }
    }

    for (const ext of [".tsx", ".ts", ".jsx", ".js", ".json", ".css", ".woff2", ".woff", ".ttf"]) {
      const candidate = filePath + ext;
      if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) {
        return candidate;
      }
    }

    return null;
  }

  return {
    name: "hash-path-resolver",
    enforce: "pre" as const,
    resolveId(source: string, importer?: string) {
      // 1. Handle @/ alias
      if (source.startsWith("@/")) {
        const full = path.join(projectRoot, "src", source.slice(2));
        const res = resolveWithExt(full);
        if (res) return res;
      }

      // 2. Handle relative imports
      if (source.startsWith("./") || source.startsWith("../")) {
        if (!importer) return null;
        const importerFile = importer.includes("?")
          ? importer.slice(0, importer.indexOf("?"))
          : importer;
        const importerDir = path.isAbsolute(importerFile)
          ? path.dirname(importerFile)
          : path.dirname(path.join(projectRoot, importerFile));

        const full = path.resolve(importerDir, source);
        const res = resolveWithExt(full);
        if (res) return res;
      }

      // 3. Handle absolute paths within workspace
      if (path.isAbsolute(source)) {
        const res = resolveWithExt(source);
        if (res) return res;
      }

      return null;
    },
  };
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [hashPathResolverPlugin(), react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
