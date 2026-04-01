/**
 * Chat panel UI component for the LLM assistant.
 *
 * Renders as a collapsible overlay inside the viewer panel.
 * Supports streaming token display, model loading progress,
 * and "Apply" buttons on generated code blocks.
 */

export interface ChatPanelOptions {
  /** Parent element to attach the panel to (viewer-panel) */
  container: HTMLElement;
  /** Called when the user submits a message */
  onSend: (message: string) => void;
  /** Called when the user clicks "Apply" on a code block */
  onApplyCode: (code: string) => void;
  /** Called when the panel is closed */
  onClose: () => void;
  /** Called when model selection changes */
  onModelChange: (modelId: string) => void;
}

interface ModelOption {
  id: string;
  label: string;
}

export class ChatPanel {
  private panel: HTMLElement;
  private messagesEl: HTMLElement;
  private inputEl: HTMLTextAreaElement;
  private sendBtn: HTMLElement;
  private statusEl: HTMLElement;
  private progressBar: HTMLElement;
  private progressFill: HTMLElement;
  private modelSelect: HTMLSelectElement;
  private options: ChatPanelOptions;
  private visible = false;
  private msgCounter = 0;

  constructor(options: ChatPanelOptions) {
    this.options = options;
    this.panel = this.buildDOM();
    options.container.appendChild(this.panel);

    this.messagesEl = this.panel.querySelector('.chat-messages')!;
    this.inputEl = this.panel.querySelector('.chat-input')!;
    this.sendBtn = this.panel.querySelector('.chat-send')!;
    this.statusEl = this.panel.querySelector('.chat-status')!;
    this.progressBar = this.panel.querySelector('.chat-progress')!;
    this.progressFill = this.panel.querySelector('.chat-progress-fill')!;
    this.modelSelect = this.panel.querySelector('.chat-model-select')!;

    this.bindEvents();
  }

  private buildDOM(): HTMLElement {
    const el = document.createElement('div');
    el.className = 'chat-panel chat-hidden';
    el.innerHTML = `
      <div class="chat-header">
        <span class="chat-title">AI Assistant</span>
        <select class="chat-model-select"></select>
        <button class="chat-close" title="Close">\u00d7</button>
      </div>
      <div class="chat-status">Not loaded</div>
      <div class="chat-progress chat-progress-hidden">
        <div class="chat-progress-fill"></div>
      </div>
      <div class="chat-messages"></div>
      <div class="chat-input-row">
        <textarea class="chat-input" placeholder="Describe a shape..." rows="2" disabled></textarea>
        <button class="chat-send" disabled>Send</button>
      </div>
    `;
    return el;
  }

  private bindEvents(): void {
    this.panel.querySelector('.chat-close')!.addEventListener('click', () => {
      this.hide();
      this.options.onClose();
    });

    this.sendBtn.addEventListener('click', () => this.submitMessage());

    this.inputEl.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        this.submitMessage();
      }
    });

    this.modelSelect.addEventListener('change', () => {
      this.options.onModelChange(this.modelSelect.value);
    });
  }

  private submitMessage(): void {
    const text = this.inputEl.value.trim();
    if (!text) return;
    this.inputEl.value = '';
    this.options.onSend(text);
  }

  // ── Public API ──────────────────────────────────────────────────

  show(): void {
    this.visible = true;
    this.panel.classList.remove('chat-hidden');
  }

  hide(): void {
    this.visible = false;
    this.panel.classList.add('chat-hidden');
  }

  isVisible(): boolean {
    return this.visible;
  }

  toggle(): void {
    if (this.visible) {
      this.hide();
      this.options.onClose();
    } else {
      this.show();
    }
  }

  /** Populate the model dropdown */
  setModelOptions(models: ModelOption[], selectedId: string): void {
    this.modelSelect.innerHTML = models
      .map((m) => `<option value="${m.id}" ${m.id === selectedId ? 'selected' : ''}>${m.label}</option>`)
      .join('');
  }

  getSelectedModel(): string {
    return this.modelSelect.value;
  }

  /** Set status text (e.g. "Ready", "Generating...") */
  setStatus(text: string): void {
    this.statusEl.textContent = text;
  }

  /** Show/update the progress bar (0..1) */
  setProgress(ratio: number, text?: string): void {
    this.progressBar.classList.remove('chat-progress-hidden');
    this.progressFill.style.width = `${Math.round(ratio * 100)}%`;
    if (text) this.statusEl.textContent = text;
  }

  hideProgress(): void {
    this.progressBar.classList.add('chat-progress-hidden');
  }

  /** Enable/disable the input area */
  setInputEnabled(enabled: boolean): void {
    this.inputEl.disabled = !enabled;
    (this.sendBtn as HTMLButtonElement).disabled = !enabled;
    if (enabled) {
      this.inputEl.focus();
    }
  }

  /** Add a user message bubble */
  addUserMessage(text: string): void {
    const bubble = document.createElement('div');
    bubble.className = 'chat-msg chat-msg-user';
    bubble.textContent = text;
    this.messagesEl.appendChild(bubble);
    this.scrollToBottom();
  }

  /** Create an empty assistant bubble for streaming. Returns its ID. */
  addAssistantMessage(): string {
    const id = `chat-msg-${++this.msgCounter}`;
    const bubble = document.createElement('div');
    bubble.className = 'chat-msg chat-msg-assistant';
    bubble.id = id;
    bubble.innerHTML = '<span class="chat-msg-content"></span>';
    this.messagesEl.appendChild(bubble);
    this.scrollToBottom();
    return id;
  }

  /** Append a token to a streaming assistant bubble */
  appendToken(bubbleId: string, token: string): void {
    const content = document.getElementById(bubbleId)?.querySelector('.chat-msg-content');
    if (content) {
      content.textContent += token;
      this.scrollToBottom();
    }
  }

  /** Finalize an assistant message, adding "Apply" button if code is detected */
  finalizeMessage(bubbleId: string, code: string | null): void {
    const bubble = document.getElementById(bubbleId);
    if (!bubble) return;

    if (code) {
      // Replace content with formatted code + apply button
      const content = bubble.querySelector('.chat-msg-content')!;
      const pre = document.createElement('pre');
      pre.className = 'chat-code';
      pre.textContent = code;
      content.textContent = '';
      content.appendChild(pre);

      const applyBtn = document.createElement('button');
      applyBtn.className = 'chat-apply-btn';
      applyBtn.textContent = 'Apply to Editor';
      applyBtn.addEventListener('click', () => {
        this.options.onApplyCode(code);
        applyBtn.textContent = 'Applied!';
        applyBtn.disabled = true;
        setTimeout(() => {
          applyBtn.textContent = 'Apply to Editor';
          applyBtn.disabled = false;
        }, 2000);
      });
      bubble.appendChild(applyBtn);
    }

    this.scrollToBottom();
  }

  /** Add an error message */
  addErrorMessage(text: string): void {
    const bubble = document.createElement('div');
    bubble.className = 'chat-msg chat-msg-error';
    bubble.textContent = text;
    this.messagesEl.appendChild(bubble);
    this.scrollToBottom();
  }

  /** Clear all messages */
  clearMessages(): void {
    this.messagesEl.innerHTML = '';
  }

  private scrollToBottom(): void {
    this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
  }
}
