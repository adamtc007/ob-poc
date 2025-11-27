import { EntityPicker } from "./components/EntityPicker";
import type {
  FormTemplate,
  SlotDefinition,
  SlotType,
  EntityType,
  EnumOption,
} from "./types";

export interface FormRendererConfig {
  container: HTMLElement;
  template: FormTemplate;
  values: Record<string, unknown>;
  onChange: (name: string, value: unknown) => void;
}

export class FormRenderer {
  private config: FormRendererConfig;
  private pickers: Map<string, EntityPicker> = new Map();
  private currentValues: Record<string, unknown>;

  constructor(config: FormRendererConfig) {
    this.config = config;
    this.currentValues = { ...config.values }; // Clone initial values
    this.render();
  }

  // Helper to update value and notify
  private setValue(name: string, value: unknown): void {
    this.currentValues[name] = value;
    this.config.onChange(name, value);
  }

  private render(): void {
    this.config.container.innerHTML = "";
    this.pickers.clear();

    for (const slot of this.config.template.slots) {
      const field = this.createField(slot);
      this.config.container.appendChild(field);
    }
  }

  private createField(slot: SlotDefinition): HTMLDivElement {
    const field = document.createElement("div");
    field.className = "form-field";
    field.dataset.slot = slot.name;

    // Label
    const label = document.createElement("label");
    label.htmlFor = `field-${slot.name}`;
    label.innerHTML = slot.label;
    if (slot.required) {
      label.innerHTML += '<span class="required-star">*</span>';
    }
    field.appendChild(label);

    // Input based on slot type
    const input = this.createInput(slot);
    field.appendChild(input);

    // Help text
    if (slot.help_text) {
      const help = document.createElement("div");
      help.className = "help-text";
      help.textContent = slot.help_text;
      field.appendChild(help);
    }

    return field;
  }

  private createInput(slot: SlotDefinition): HTMLElement {
    const value = this.config.values[slot.name];
    const slotType = slot.slot_type;

    switch (slotType.type) {
      case "text":
        return this.createTextInput(slot, slotType, value);

      case "enum":
        return this.createEnumSelect(slot, slotType, value);

      case "entity_ref":
        return this.createEntityPicker(slot, slotType, value);

      case "country":
        return this.createCountrySelect(slot, value);

      case "date":
        return this.createDateInput(slot, value);

      case "percentage":
      case "integer":
      case "decimal":
        return this.createNumberInput(slot, slotType, value);

      case "boolean":
        return this.createCheckbox(slot, value);

      default:
        return this.createTextInput(slot, { type: "text" }, value);
    }
  }

  private createTextInput(
    slot: SlotDefinition,
    slotType: { type: "text"; max_length?: number; multiline?: boolean },
    value: unknown,
  ): HTMLElement {
    if (slotType.multiline) {
      const textarea = document.createElement("textarea");
      textarea.id = `field-${slot.name}`;
      textarea.value = String(value ?? "");
      textarea.placeholder = slot.placeholder ?? "";
      if (slotType.max_length) {
        textarea.maxLength = slotType.max_length;
      }
      textarea.addEventListener("input", () => {
        this.setValue(slot.name, textarea.value);
      });
      return textarea;
    }

    const input = document.createElement("input");
    input.type = "text";
    input.id = `field-${slot.name}`;
    input.value = String(value ?? "");
    input.placeholder = slot.placeholder ?? "";
    if (slotType.max_length) {
      input.maxLength = slotType.max_length;
    }
    input.addEventListener("input", () => {
      this.setValue(slot.name, input.value);
    });
    return input;
  }

  private createEnumSelect(
    slot: SlotDefinition,
    slotType: { type: "enum"; options: EnumOption[] },
    value: unknown,
  ): HTMLSelectElement {
    const select = document.createElement("select");
    select.id = `field-${slot.name}`;

    // Add empty option if not required
    if (!slot.required) {
      const empty = document.createElement("option");
      empty.value = "";
      empty.textContent = "-- Select --";
      select.appendChild(empty);
    }

    for (const opt of slotType.options) {
      const option = document.createElement("option");
      option.value = opt.value;
      option.textContent = opt.label;
      if (opt.value === value) {
        option.selected = true;
      }
      select.appendChild(option);
    }

    select.addEventListener("change", () => {
      this.setValue(slot.name, select.value);
    });

    return select;
  }

  private createEntityPicker(
    slot: SlotDefinition,
    slotType: {
      type: "entity_ref";
      allowed_types: EntityType[];
      allow_create: boolean;
    },
    _value: unknown,
  ): HTMLElement {
    const container = document.createElement("div");
    container.id = `field-${slot.name}`;

    console.log(
      "[FormRenderer] Creating EntityPicker for slot:",
      slot.name,
      "with types:",
      slotType.allowed_types,
    );

    const picker = new EntityPicker({
      container,
      allowedTypes: slotType.allowed_types,
      allowCreate: slotType.allow_create,
      placeholder:
        slot.placeholder ?? `Search ${slotType.allowed_types.join(", ")}...`,
      onSelect: (entity) => {
        this.setValue(slot.name, entity?.id ?? null);
      },
      onCreate: (type) => {
        // TODO: Open create dialog
        console.log("Create new:", type);
      },
    });

    this.pickers.set(slot.name, picker);

    return container;
  }

  private createCountrySelect(
    slot: SlotDefinition,
    value: unknown,
  ): HTMLSelectElement {
    // Common countries - could be expanded
    const countries = [
      { code: "GB", name: "United Kingdom" },
      { code: "US", name: "United States" },
      { code: "DE", name: "Germany" },
      { code: "FR", name: "France" },
      { code: "CH", name: "Switzerland" },
      { code: "LU", name: "Luxembourg" },
      { code: "IE", name: "Ireland" },
      { code: "NL", name: "Netherlands" },
      { code: "SG", name: "Singapore" },
      { code: "HK", name: "Hong Kong" },
      { code: "JE", name: "Jersey" },
      { code: "GG", name: "Guernsey" },
      { code: "KY", name: "Cayman Islands" },
      { code: "VG", name: "British Virgin Islands" },
    ];

    const select = document.createElement("select");
    select.id = `field-${slot.name}`;

    const empty = document.createElement("option");
    empty.value = "";
    empty.textContent = "-- Select Country --";
    select.appendChild(empty);

    for (const c of countries) {
      const option = document.createElement("option");
      option.value = c.code;
      option.textContent = `${c.name} (${c.code})`;
      if (c.code === value) {
        option.selected = true;
      }
      select.appendChild(option);
    }

    select.addEventListener("change", () => {
      this.setValue(slot.name, select.value);
    });

    return select;
  }

  private createDateInput(
    slot: SlotDefinition,
    value: unknown,
  ): HTMLInputElement {
    const input = document.createElement("input");
    input.type = "date";
    input.id = `field-${slot.name}`;
    input.value = String(value ?? "");
    input.addEventListener("change", () => {
      this.setValue(slot.name, input.value);
    });
    return input;
  }

  private createNumberInput(
    slot: SlotDefinition,
    slotType: SlotType,
    value: unknown,
  ): HTMLInputElement {
    const input = document.createElement("input");
    input.type = "number";
    input.id = `field-${slot.name}`;
    input.value = String(value ?? "");
    input.placeholder = slot.placeholder ?? "";

    if (slotType.type === "percentage") {
      input.min = "0";
      input.max = "100";
      input.step = "0.01";
    } else if (slotType.type === "integer") {
      input.step = "1";
      if (slotType.min !== undefined) input.min = String(slotType.min);
      if (slotType.max !== undefined) input.max = String(slotType.max);
    } else {
      input.step = "0.01";
    }

    input.addEventListener("input", () => {
      const numValue =
        slotType.type === "integer"
          ? parseInt(input.value, 10)
          : parseFloat(input.value);
      this.setValue(slot.name, isNaN(numValue) ? null : numValue);
    });

    return input;
  }

  private createCheckbox(slot: SlotDefinition, value: unknown): HTMLElement {
    const wrapper = document.createElement("div");
    wrapper.className = "checkbox-wrapper";

    const input = document.createElement("input");
    input.type = "checkbox";
    input.id = `field-${slot.name}`;
    input.checked = Boolean(value);
    input.addEventListener("change", () => {
      this.setValue(slot.name, input.checked);
    });

    const label = document.createElement("label");
    label.htmlFor = `field-${slot.name}`;
    label.textContent = slot.label;

    wrapper.appendChild(input);
    wrapper.appendChild(label);

    return wrapper;
  }

  // Public: Get all current values
  getValues(): Record<string, unknown> {
    return { ...this.currentValues };
  }

  // Public: Check if all required fields are filled
  isValid(): boolean {
    for (const slot of this.config.template.slots) {
      if (slot.required) {
        const value = this.currentValues[slot.name];
        if (value === undefined || value === null || value === "") {
          return false;
        }
      }
    }
    return true;
  }

  // Public: Destroy and cleanup
  destroy(): void {
    this.pickers.clear();
    this.config.container.innerHTML = "";
  }
}
