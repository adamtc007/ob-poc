// Entity Finder Modal - Resolves unresolved EntityRefs via EntityGateway
//
// This modal is used by both:
// 1. Agent chat - when disambiguation is needed
// 2. AST panel - when clicking unresolved EntityRefs
//
// It queries /api/agent/complete for fuzzy entity search.

export interface EntityMatch {
  value: string; // UUID or code
  label: string; // Display name
  detail?: string; // Additional context (type, jurisdiction)
  score: number; // Relevance score
}

export interface ResolveContext {
  statementIndex: number;
  argKey: string;
  entityType: string;
  searchText: string;
}

export type EntityFinderCallback = (
  context: ResolveContext,
  selected: EntityMatch,
) => void;

export class EntityFinderModal {
  private dialog: HTMLDialogElement;
  private searchInput: HTMLInputElement;
  private resultsEl: HTMLElement;
  private contextEl: HTMLElement;
  private closeBtn: HTMLButtonElement;

  private context: ResolveContext | null = null;
  private results: EntityMatch[] = [];
  private selectedIndex: number = 0;
  private searchTimeout: number | null = null;
  private onSelect: EntityFinderCallback | null = null;

  constructor() {
    this.dialog = document.getElementById(
      "entity-finder-modal",
    ) as HTMLDialogElement;
    this.searchInput = document.getElementById(
      "entity-finder-search",
    ) as HTMLInputElement;
    this.resultsEl = document.getElementById(
      "entity-finder-results",
    ) as HTMLElement;
    this.contextEl = document.getElementById(
      "entity-finder-context",
    ) as HTMLElement;
    this.closeBtn = this.dialog.querySelector(
      ".modal-close",
    ) as HTMLButtonElement;

    this.setupEventListeners();
  }

  private setupEventListeners() {
    // Close button
    this.closeBtn.addEventListener("click", () => this.close());

    // Click outside to close
    this.dialog.addEventListener("click", (e) => {
      if (e.target === this.dialog) {
        this.close();
      }
    });

    // Search input
    this.searchInput.addEventListener("input", () => {
      this.debounceSearch();
    });

    // Keyboard navigation
    this.searchInput.addEventListener("keydown", (e) => {
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          this.selectNext();
          break;
        case "ArrowUp":
          e.preventDefault();
          this.selectPrevious();
          break;
        case "Enter":
          e.preventDefault();
          this.confirmSelection();
          break;
        case "Escape":
          e.preventDefault();
          this.close();
          break;
      }
    });

    // Click on result
    this.resultsEl.addEventListener("click", (e) => {
      const resultEl = (e.target as HTMLElement).closest(
        ".search-result",
      ) as HTMLElement;
      if (resultEl) {
        const index = parseInt(resultEl.dataset.index || "0", 10);
        this.selectedIndex = index;
        this.confirmSelection();
      }
    });
  }

  open(context: ResolveContext, onSelect: EntityFinderCallback) {
    this.context = context;
    this.onSelect = onSelect;
    this.results = [];
    this.selectedIndex = 0;

    // Set context display
    this.contextEl.textContent = `${context.entityType}: "${context.searchText}"`;

    // Pre-fill search with current value
    this.searchInput.value = context.searchText;

    // Clear results
    this.resultsEl.innerHTML =
      '<div class="search-placeholder">Searching...</div>';

    // Show modal
    this.dialog.showModal();
    this.searchInput.focus();
    this.searchInput.select();

    // Trigger initial search
    this.performSearch();
  }

  close() {
    this.dialog.close();
    this.context = null;
    this.onSelect = null;
  }

  private debounceSearch() {
    if (this.searchTimeout) {
      clearTimeout(this.searchTimeout);
    }
    this.searchTimeout = window.setTimeout(() => {
      this.performSearch();
    }, 200);
  }

  private async performSearch() {
    if (!this.context) return;

    const query = this.searchInput.value.trim();
    if (query.length < 1) {
      this.resultsEl.innerHTML =
        '<div class="search-placeholder">Type to search...</div>';
      return;
    }

    try {
      const response = await fetch("/api/agent/complete", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          entity_type: this.context.entityType,
          query: query,
          limit: 10,
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data = await response.json();
      this.results = data.items || [];
      this.selectedIndex = 0;
      this.renderResults();
    } catch (err) {
      console.error("Entity search failed:", err);
      this.resultsEl.innerHTML = `<div class="search-error">Search failed: ${err}</div>`;
    }
  }

  private renderResults() {
    if (this.results.length === 0) {
      this.resultsEl.innerHTML =
        '<div class="search-placeholder">No matches found</div>';
      return;
    }

    const html = this.results
      .map((result, index) => {
        const selected = index === this.selectedIndex ? "selected" : "";
        const detail = result.detail
          ? `<span class="result-detail">${result.detail}</span>`
          : "";
        const score = result.score
          ? `<span class="result-score">${(result.score * 100).toFixed(0)}%</span>`
          : "";

        return `
                <div class="search-result ${selected}" data-index="${index}">
                    <span class="result-label">${this.escapeHtml(result.label)}</span>
                    ${detail}
                    ${score}
                </div>
            `;
      })
      .join("");

    this.resultsEl.innerHTML = html;
  }

  private selectNext() {
    if (this.results.length === 0) return;
    this.selectedIndex = (this.selectedIndex + 1) % this.results.length;
    this.renderResults();
    this.scrollSelectedIntoView();
  }

  private selectPrevious() {
    if (this.results.length === 0) return;
    this.selectedIndex =
      (this.selectedIndex - 1 + this.results.length) % this.results.length;
    this.renderResults();
    this.scrollSelectedIntoView();
  }

  private scrollSelectedIntoView() {
    const selected = this.resultsEl.querySelector(".search-result.selected");
    selected?.scrollIntoView({ block: "nearest" });
  }

  private confirmSelection() {
    if (this.results.length === 0 || !this.context || !this.onSelect) return;

    const selected = this.results[this.selectedIndex];
    this.onSelect(this.context, selected);
    this.close();
  }

  private escapeHtml(text: string): string {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  getContext(): ResolveContext | null {
    return this.context;
  }
}
