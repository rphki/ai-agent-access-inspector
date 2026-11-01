import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { I18nProvider } from "./i18n";
import type { Language } from "./i18n";
import { LINUX_BASIC, MALFORMED, NO_CONFIG, NOT_DETECTED } from "./test/fixtures";
import type { StoredScan } from "./types/report";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

/** Mocks the backend: `scan` is returned by `run_scan`; no stored scan exists. */
function backend(scan: StoredScan | Error, overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command in overrides) {
      const value = overrides[command];
      if (value instanceof Object && "kind" in value) throw value;
      return value;
    }
    switch (command) {
      case "get_app_info":
        return { name: "ai-agent-access-inspector", version: "0.2.0", operatingSystem: "linux" };
      case "get_scan_details":
        return null;
      case "run_scan":
        if (scan instanceof Error) throw { kind: "scanFailed", detail: scan.message };
        return scan;
      default:
        return undefined;
    }
  });
}

function renderApp(language: Language = "en") {
  return render(
    <I18nProvider language={language}>
      <App />
    </I18nProvider>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("App", () => {
  it("scans on start and lists the findings", async () => {
    backend(LINUX_BASIC);
    renderApp();
    expect(await screen.findByText("10 of 10 findings")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("run_scan");
    const table = screen.getByRole("table");
    expect(within(table).getByRole("button", { name: "Show details for /etc/shadow" })).toBeInTheDocument();
    expect(within(table).getAllByText("MCP server files").length).toBeGreaterThan(0);
    expect(screen.getByText("Scan completed.")).toBeInTheDocument();
  });

  it("reuses a stored scan instead of scanning again", async () => {
    backend(LINUX_BASIC, { get_scan_details: LINUX_BASIC });
    renderApp();
    await screen.findByText("10 of 10 findings");
    expect(invokeMock).not.toHaveBeenCalledWith("run_scan");
  });

  it("filters the results", async () => {
    backend(LINUX_BASIC);
    renderApp();
    await screen.findByText("10 of 10 findings");
    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText("Effect"), "deny");
    expect(screen.getByText("3 of 10 findings")).toBeInTheDocument();
    await user.click(screen.getByLabelText("Existing paths only"));
    expect(screen.getByText("1 of 10 findings")).toBeInTheDocument();
    await user.click(screen.getByLabelText("Warnings only"));
    expect(screen.getByText("No findings match the filters.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reset filters" }));
    expect(screen.getByText("10 of 10 findings")).toBeInTheDocument();
  });

  it("shows all details of a finding and its actions", async () => {
    backend(LINUX_BASIC);
    renderApp();
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Show details for /home/alice/app" }));

    const detail = screen.getByRole("region", { name: /Details:/ });
    expect(within(detail).getByRole("heading", { level: 2 })).toHaveFocus();
    expect(within(detail).getByText(/Known project/)).toBeInTheDocument();
    expect(within(detail).getByText("heuristic (undocumented source)")).toBeInTheDocument();
    expect(within(detail).getByText("/projects/~1home~1alice~1app")).toBeInTheDocument();
    expect(within(detail).getAllByText(/^Contains:/).length).toBe(2);

    await user.click(within(detail).getByRole("button", { name: "Show in file manager" }));
    expect(invokeMock).toHaveBeenCalledWith("open_path_in_file_manager", { path: "/home/alice/app" });
    await user.click(within(detail).getByRole("button", { name: "Open configuration file" }));
    expect(invokeMock).toHaveBeenCalledWith("open_source_file", { path: "/home/alice/.claude.json" });

    const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    await user.click(within(detail).getByRole("button", { name: "Copy finding as JSON" }));
    expect(JSON.parse(writeText.mock.calls[0][0])).toMatchObject({ id: "claude-code-82109f3dda6ade2f" });
    expect(within(detail).getByText("Copied to the clipboard.")).toBeInTheDocument();

    // Related findings are links to their details.
    const related = within(detail).getAllByRole("button", { name: /\/home\/alice\/app\/src/ })[0];
    await user.click(related);
    expect(screen.getByRole("region", { name: /Details: \/home\/alice\/app\/src/ })).toBeInTheDocument();
  });

  it("shows warnings with their severity", async () => {
    backend(LINUX_BASIC);
    renderApp();
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Show details for Bash" }));
    const detail = screen.getByRole("region", { name: /Details:/ });
    expect(within(detail).getByText(/No sandbox is configured/)).toBeInTheDocument();
  });

  it("explains a refused open action", async () => {
    backend(LINUX_BASIC, { open_path_in_file_manager: { kind: "pathNotInReport", detail: "/home/alice/app" } });
    renderApp();
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Show details for /home/alice/app" }));
    await user.click(screen.getByRole("button", { name: "Show in file manager" }));
    expect(await screen.findByText(/not part of the last scan/)).toBeInTheDocument();
  });

  it("shows the empty states", async () => {
    backend(NOT_DETECTED);
    const { unmount } = renderApp();
    expect(await screen.findByText("No supported agent was found on this computer.")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    unmount();

    backend(NO_CONFIG);
    renderApp();
    expect(await screen.findByText(/Claude Code was found, but its configuration grants no relevant access/)).toBeInTheDocument();
  });

  it("shows a partially failed scan with its diagnostics", async () => {
    backend(MALFORMED);
    renderApp();
    expect(await screen.findByText(/only partially successful/)).toBeInTheDocument();
    expect(screen.getByText("1 configuration file(s) could not be read.")).toBeInTheDocument();
    expect(screen.getByText(/configuration file\(s\) contain invalid values/)).toBeInTheDocument();
    expect(screen.getByText(/The file is not valid JSON/)).toBeVisible();
  });

  it("shows an internal error and allows a new scan", async () => {
    backend(new Error("thread panicked"));
    renderApp();
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The scan failed unexpectedly. Details: thread panicked");
    backend(LINUX_BASIC);
    await userEvent.setup().click(within(alert).getByRole("button", { name: "Scan again" }));
    expect(await screen.findByText("10 of 10 findings")).toBeInTheDocument();
  });

  it("switches the language", async () => {
    backend(LINUX_BASIC);
    renderApp("de");
    expect(await screen.findByText("10 von 10 Befunden")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Details zu /home/alice/app anzeigen" }));
    expect(screen.getByText(/Bekanntes Projekt:/)).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Sprache"), "en");
    expect(screen.getByText(/Known project:/)).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
  });
});
