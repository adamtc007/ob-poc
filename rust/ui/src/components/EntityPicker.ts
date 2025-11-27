import { api } from "../api";
import type {
  EntityMatch,
  EntityType,
  EntitySearchResponse,
} from "../types";

export interface EntityPickerConfig {
  container: HTMLElement;
  allowedTypes: EntityType[];
  allowCreate: boolean;
  placeholder?: string;
  onSelect: (entity: EntityMatch | null) => void;
  onCreate?: (type: EntityType) => void;
}

export class EntityPicker {
  private config: EntityPickerConfig;
  private input: HTMLInputElement;
  private dropdown: HTMLDivElement;
  private selectedEntity: EntityMatch | null = null;
  private debounceTimer: number | null = null;
  private isOpen = false;

  constructor(config: EntityPickerConfig) {
    this.config = config;
    this.input = this.createInput();
    this.dropdown = this.createDropdown();
    this.render();
    this.bindEvents();
  }

  private createInput(): HTMLInputElement {
    const input = document.createElement("input");
    input.type = "text";
    input.className = "entity-picker-input";
    input.placeholder = this.config.placeholder || "Search...";
    return input;
  }

  private createDropdown(): HTMLDivElement {
    const dropdown = document.createElement("div");
    dropdown.className = "entity-picker-dropdown";
    dropdown.style.display = "none";
    return dropdown;
  }

  private render(): void {
    const wrapper = document.createElement("div");
    wrapper.className = "entity-picker";
    wrapper.appendChild(this.input);
    wrapper.appendChild(this.dropdown);
    this.config.container.appendChild(wrapper);
  }

  private bindEvents(): void {
    this.input.addEventListener("input", () => this.handleInput());
    this.input.addEventListener("focus", () => this.handleFocus());
    this.input.addEventListener("blur", () => {
      // Delay close to allow clicking on dropdown items
      // Must be longer than debounce (200ms) to avoid race condition
      setTimeout(() => this.close(), 300);
    });
    this.input.addEventListener("keydown", (e) => this.handleKeydown(e));
  }

  private handleInput(): void {
    const query = this.input.value.trim();
    console.log('[EntityPicker] handleInput triggered, query:', query);

    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }

    if (query.length < 2) {
      this.close();
      return;
    }

    this.debounceTimer = window.setTimeout(() => {
      this.search(query);
    }, 200);
  }

  private handleFocus(): void {
    if (this.input.value.trim().length >= 2) {
      this.search(this.input.value.trim());
    }
  }

  private handleKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      this.close();
    } else if (e.key === "ArrowDown" && this.isOpen) {
      e.preventDefault();
      this.focusNextItem();
    } else if (e.key === "ArrowUp" && this.isOpen) {
      e.preventDefault();
      this.focusPrevItem();
    } else if (e.key === "Enter" && this.isOpen) {
      e.preventDefault();
      this.selectFocusedItem();
    }
  }

  private async search(query: string): Promise<void> {
    try {
      console.log('[EntityPicker] Searching:', { query, types: this.config.allowedTypes });
      const response = await api.searchEntities({
        q: query,
        types: this.config.allowedTypes,
        limit: 10,
      });
      console.log('[EntityPicker] Results:', response);
      this.renderResults(response);
    } catch (err) {
      console.error("[EntityPicker] Search failed:", err);
      this.renderError("Search failed");
    }
  }

  private renderResults(response: EntitySearchResponse): void {
    this.dropdown.innerHTML = "";

    if (response.results.length === 0) {
      const empty = document.createElement("div");
      empty.className = "entity-picker-empty";
      empty.textContent = "No results found";
      this.dropdown.appendChild(empty);
    } else {
      response.results.forEach((entity, index) => {
        const item = this.createResultItem(entity, index);
        this.dropdown.appendChild(item);
      });

      if (response.truncated) {
        const more = document.createElement("div");
        more.className = "entity-picker-more";
        more.textContent = `${response.total - response.results.length} more results...`;
        this.dropdown.appendChild(more);
      }
    }

    // Add create options
    if (this.config.allowCreate) {
      const divider = document.createElement("div");
      divider.className = "entity-picker-divider";
      this.dropdown.appendChild(divider);

      this.config.allowedTypes.forEach((type) => {
        if (type !== "CBU") {
          // Can't create CBU inline
          const createItem = document.createElement("div");
          createItem.className = "entity-picker-item entity-picker-create";
          createItem.innerHTML = `<span class="create-icon">+</span> Create New ${type.toLowerCase()}`;
          createItem.addEventListener("click", () => {
            if (this.config.onCreate) {
              this.config.onCreate(type);
            }
            this.close();
          });
          this.dropdown.appendChild(createItem);
        }
      });
    }

    this.open();
  }

  private createResultItem(
    entity: EntityMatch,
    index: number,
  ): HTMLDivElement {
    const item = document.createElement("div");
    item.className = "entity-picker-item";
    item.dataset.index = String(index);
    item.dataset.id = entity.id;

    const typeLabel = document.createElement("span");
    typeLabel.className = `entity-type entity-type-${entity.entity_type.toLowerCase()}`;
    typeLabel.textContent = entity.entity_type;

    const name = document.createElement("span");
    name.className = "entity-name";
    name.textContent = entity.display_name;

    const subtitle = document.createElement("span");
    subtitle.className = "entity-subtitle";
    subtitle.textContent = [entity.subtitle, entity.detail]
      .filter(Boolean)
      .join(" - ");

    item.appendChild(typeLabel);
    item.appendChild(name);
    item.appendChild(subtitle);

    item.addEventListener("click", () => this.selectEntity(entity));

    return item;
  }

  private renderError(message: string): void {
    this.dropdown.innerHTML = "";
    const error = document.createElement("div");
    error.className = "entity-picker-error";
    error.textContent = message;
    this.dropdown.appendChild(error);
    this.open();
  }

  private selectEntity(entity: EntityMatch): void {
    this.selectedEntity = entity;
    this.input.value = entity.display_name;
    this.config.onSelect(entity);
    this.close();
  }

  private open(): void {
    this.dropdown.style.display = "block";
    this.isOpen = true;
  }

  private close(): void {
    this.dropdown.style.display = "none";
    this.isOpen = false;
  }

  private focusNextItem(): void {
    const items = this.dropdown.querySelectorAll(".entity-picker-item");
    const focused = this.dropdown.querySelector(".entity-picker-item.focused");
    const currentIndex = focused
      ? parseInt(focused.getAttribute("data-index") || "-1")
      : -1;
    const nextIndex = Math.min(currentIndex + 1, items.length - 1);

    items.forEach((item, i) => {
      item.classList.toggle("focused", i === nextIndex);
    });
  }

  private focusPrevItem(): void {
    const items = this.dropdown.querySelectorAll(".entity-picker-item");
    const focused = this.dropdown.querySelector(".entity-picker-item.focused");
    const currentIndex = focused
      ? parseInt(focused.getAttribute("data-index") || "0")
      : 0;
    const prevIndex = Math.max(currentIndex - 1, 0);

    items.forEach((item, i) => {
      item.classList.toggle("focused", i === prevIndex);
    });
  }

  private selectFocusedItem(): void {
    const focused = this.dropdown.querySelector(".entity-picker-item.focused");
    if (focused) {
      (focused as HTMLElement).click();
    }
  }

  // Public methods
  getValue(): EntityMatch | null {
    return this.selectedEntity;
  }

  setValue(entity: EntityMatch | null): void {
    this.selectedEntity = entity;
    this.input.value = entity?.display_name || "";
  }

  clear(): void {
    this.selectedEntity = null;
    this.input.value = "";
    this.config.onSelect(null);
  }
}
