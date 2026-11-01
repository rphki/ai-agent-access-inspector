import { CATALOGS, translate, translateMessage } from "./i18n";

const placeholders = (text: string) => [...text.matchAll(/\{([A-Za-z0-9_]+)\}/g)].map((m) => m[1]).sort();

describe("catalogs", () => {
  it("German covers every English key with the same placeholders", () => {
    for (const [key, english] of Object.entries(CATALOGS.en)) {
      const german = CATALOGS.de[key];
      expect(german, key).toBeDefined();
      expect(placeholders(german), key).toEqual(placeholders(english));
    }
  });

  it("has no German keys without an English original", () => {
    expect(Object.keys(CATALOGS.de).filter((key) => !(key in CATALOGS.en))).toEqual([]);
  });
});

describe("translate", () => {
  it("fills parameters without translating them", () => {
    expect(translate("de", "warning.wholeDrive", { path: "D:\\" })).toBe("Gewährt das gesamte Laufwerk D:\\.");
  });

  it("keeps unknown placeholders visible", () => {
    expect(translate("en", "warning.wholeDrive")).toBe("Grants the whole drive {path}.");
  });

  it("falls back to the core's English text for unknown keys", () => {
    const message = { key: "warning.fromTheFuture", args: {}, text: "A newer core text." };
    expect(translateMessage("de", message)).toBe("A newer core text.");
  });
});
