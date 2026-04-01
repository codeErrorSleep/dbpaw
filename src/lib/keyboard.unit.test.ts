import { describe, expect, test, mock } from "bun:test";

// Setup minimal DOM mock for keyboard tests
const mockElement = (tagName: string, props: Record<string, any> = {}) => {
  const element = {
    tagName: tagName.toUpperCase(),
    contentEditable: props.contentEditable || "false",
    isContentEditable: props.isContentEditable || false,
    children: [] as any[],
    parentElement: props.parentElement || null,
    appendChild: mock((child: any) => {
      element.children.push(child);
      child.parentElement = element;
      return child;
    }),
    setAttribute: mock((name: string, value: string) => {
      (element as any)[name] = value;
    }),
    getAttribute: mock((name: string) => (element as any)[name]),
    closest: mock((selector: string) => {
      // Check each selector pattern
      const selectors = selector.split(",").map((s) => s.trim());

      for (const sel of selectors) {
        if (sel.startsWith(".")) {
          // Class selector
          const className = sel.slice(1);
          if (element.className?.includes(className)) {
            return element;
          }
          // Check parent chain
          let parent = element.parentElement;
          while (parent) {
            if (parent.className?.includes(className)) {
              return parent;
            }
            parent = parent.parentElement;
          }
        } else if (sel.startsWith("[")) {
          // Attribute selector like [contenteditable='true'] or [role='textbox']
          const match = sel.match(/\[(\w+)(?:=['"]([^'"]+)['"])?\]/);
          if (match) {
            const attr = match[1];
            const value = match[2];
            if (value && (element as any)[attr] === value) {
              return element;
            }
          }
        } else {
          // Tag selector
          if (element.tagName.toLowerCase() === sel.toLowerCase()) {
            return element;
          }
        }
      }
      return null;
    }),
    className: props.className || "",
    role: props.role,
  } as any;
  return element;
};

const mockDocument = {
  createElement: mock(mockElement),
  createTextNode: mock((text: string) => ({
    nodeType: 3,
    textContent: text,
    parentElement: null,
  })),
  body: mockElement("body"),
  activeElement: null as any,
};

Object.defineProperty(global, "document", {
  value: mockDocument,
  writable: true,
});

// Mock HTMLElement for instanceof check
const MockHTMLElement = function () {};
(global as any).HTMLElement = MockHTMLElement;

// Mock Node for instanceof check
const MockNode = function () {};
(global as any).Node = MockNode;

const loadKeyboard = async () =>
  import(`./keyboard?test=${Date.now()}-${Math.random()}`);

describe("keyboard", () => {
  test("isModKey should return true when metaKey is pressed", async () => {
    const keyboard = await loadKeyboard();
    expect(keyboard.isModKey({ metaKey: true, ctrlKey: false } as any)).toBe(
      true,
    );
  });

  test("isModKey should return true when ctrlKey is pressed", async () => {
    const keyboard = await loadKeyboard();
    expect(keyboard.isModKey({ metaKey: false, ctrlKey: true } as any)).toBe(
      true,
    );
  });

  test("isModKey should return false when neither key is pressed", async () => {
    const keyboard = await loadKeyboard();
    expect(keyboard.isModKey({ metaKey: false, ctrlKey: false } as any)).toBe(
      false,
    );
  });

  test("isEditableTarget should return false for null", async () => {
    const keyboard = await loadKeyboard();
    expect(keyboard.isEditableTarget(null)).toBe(false);
  });

  test("isEditableTarget should return true for input", async () => {
    const keyboard = await loadKeyboard();
    const input = mockElement("input");
    // Make it pass instanceof check
    Object.setPrototypeOf(input, MockHTMLElement.prototype);
    expect(keyboard.isEditableTarget(input)).toBe(true);
  });

  test("isEditableTarget should return true for textarea", async () => {
    const keyboard = await loadKeyboard();
    const textarea = mockElement("textarea");
    Object.setPrototypeOf(textarea, MockHTMLElement.prototype);
    expect(keyboard.isEditableTarget(textarea)).toBe(true);
  });

  test("isEditableTarget should return true for select", async () => {
    const keyboard = await loadKeyboard();
    const select = mockElement("select");
    Object.setPrototypeOf(select, MockHTMLElement.prototype);
    expect(keyboard.isEditableTarget(select)).toBe(true);
  });

  test("isEditableTarget should return true for contentEditable", async () => {
    const keyboard = await loadKeyboard();
    const div = mockElement("div", {
      contentEditable: "true",
      isContentEditable: true,
    });
    Object.setPrototypeOf(div, MockHTMLElement.prototype);
    expect(keyboard.isEditableTarget(div)).toBe(true);
  });

  test("isEditableTarget should return true for role=textbox", async () => {
    const keyboard = await loadKeyboard();
    const div = mockElement("div", { role: "textbox" });
    Object.setPrototypeOf(div, MockHTMLElement.prototype);
    expect(keyboard.isEditableTarget(div)).toBe(true);
  });

  test("isEditableTarget should return true for .cm-editor", async () => {
    const keyboard = await loadKeyboard();
    const cmEditor = mockElement("div", { className: "cm-editor" });
    Object.setPrototypeOf(cmEditor, MockHTMLElement.prototype);
    const child = mockElement("span");
    Object.setPrototypeOf(child, MockHTMLElement.prototype);
    cmEditor.appendChild(child);
    expect(keyboard.isEditableTarget(child)).toBe(true);
  });

  test("isEditableTarget should return true for .cm-content", async () => {
    const keyboard = await loadKeyboard();
    const cmContent = mockElement("div", { className: "cm-content" });
    Object.setPrototypeOf(cmContent, MockHTMLElement.prototype);
    const child = mockElement("span");
    Object.setPrototypeOf(child, MockHTMLElement.prototype);
    cmContent.appendChild(child);
    expect(keyboard.isEditableTarget(child)).toBe(true);
  });

  test("isEditableTarget should return false for regular div", async () => {
    const keyboard = await loadKeyboard();
    const div = mockElement("div");
    Object.setPrototypeOf(div, MockHTMLElement.prototype);
    expect(keyboard.isEditableTarget(div)).toBe(false);
  });

  test("isEditableTarget should return false for button", async () => {
    const keyboard = await loadKeyboard();
    const button = mockElement("button");
    Object.setPrototypeOf(button, MockHTMLElement.prototype);
    expect(keyboard.isEditableTarget(button)).toBe(false);
  });

  test("shouldIgnoreGlobalShortcut should return true when target is editable", async () => {
    const keyboard = await loadKeyboard();
    const input = mockElement("input");
    Object.setPrototypeOf(input, MockHTMLElement.prototype);
    mockDocument.activeElement = mockDocument.body;

    const mockEvent = {
      key: "k",
      ctrlKey: true,
      target: input,
    } as unknown as KeyboardEvent;

    expect(keyboard.shouldIgnoreGlobalShortcut(mockEvent)).toBe(true);
  });

  test("shouldIgnoreGlobalShortcut should return true when activeElement is editable", async () => {
    const keyboard = await loadKeyboard();
    const input = mockElement("input");
    Object.setPrototypeOf(input, MockHTMLElement.prototype);
    mockDocument.activeElement = input;

    const mockEvent = {
      key: "k",
      ctrlKey: true,
      target: mockDocument.body,
    } as unknown as KeyboardEvent;

    expect(keyboard.shouldIgnoreGlobalShortcut(mockEvent)).toBe(true);
  });

  test("shouldIgnoreGlobalShortcut should return false when neither is editable", async () => {
    const keyboard = await loadKeyboard();
    const div = mockElement("div");
    Object.setPrototypeOf(div, MockHTMLElement.prototype);
    mockDocument.activeElement = mockDocument.body;

    const mockEvent = {
      key: "k",
      ctrlKey: true,
      target: div,
    } as unknown as KeyboardEvent;

    expect(keyboard.shouldIgnoreGlobalShortcut(mockEvent)).toBe(false);
  });

  test("shouldIgnoreGlobalShortcut should handle null target", async () => {
    const keyboard = await loadKeyboard();
    mockDocument.activeElement = mockDocument.body;

    const mockEvent = {
      key: "k",
      ctrlKey: true,
      target: null,
    } as unknown as KeyboardEvent;

    expect(keyboard.shouldIgnoreGlobalShortcut(mockEvent)).toBe(false);
  });
});
