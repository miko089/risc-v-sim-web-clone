class RISCVSimulator {
    constructor() {
        this.initializeEventListeners();
        this.updateLineNumbers();
    }

    getCodeTextarea() {
        return document.getElementById('code') || document.getElementById('file');
    }

    initializeEventListeners() {
        const form = document.getElementById('simulator-form');
        const codeTextarea = this.getCodeTextarea();
        const clearBtn = document.getElementById('clear-btn');

        if (form) {
            form.addEventListener('submit', (e) => this.handleSubmit(e));
        }

        if (codeTextarea) {
            codeTextarea.addEventListener('input', () => this.updateLineNumbers());
            codeTextarea.addEventListener('scroll', () => this.syncLineNumbers());
            codeTextarea.addEventListener('keydown', (e) => this.handleTabKey(e));
        }

        if (clearBtn) {
            clearBtn.addEventListener('click', () => this.clearForm());
        }
    }

    updateLineNumbers() {
        const textarea = this.getCodeTextarea();
        const lineNumbers = document.querySelector('.line-numbers');
        if (!textarea || !lineNumbers) {
            return;
        }

        const lines = textarea.value.split('\n').length;
        
        let numbers = '';
        for (let i = 1; i <= lines; i++) {
            numbers += i + '\n';
        }
        lineNumbers.textContent = numbers;
        lineNumbers.style.height = `${textarea.clientHeight}px`;
        lineNumbers.scrollTop = textarea.scrollTop;
    }

    syncLineNumbers() {
        const textarea = this.getCodeTextarea();
        const lineNumbers = document.querySelector('.line-numbers');
        if (!textarea || !lineNumbers) {
            return;
        }

        lineNumbers.style.height = `${textarea.clientHeight}px`;
        lineNumbers.scrollTop = textarea.scrollTop;
    }

    handleTabKey(e) {
        if (e.key === 'Tab') {
            e.preventDefault();
            const textarea = e.target;
            const start = textarea.selectionStart;
            const end = textarea.selectionEnd;
            
            textarea.value = textarea.value.substring(0, start) + '    ' + textarea.value.substring(end);
            textarea.selectionStart = textarea.selectionEnd = start + 4;
            
            this.updateLineNumbers();
        }
    }

    async handleSubmit(e) {
        e.preventDefault();
        
        const formData = new FormData(e.target);
        const ticks = formData.get('ticks');
        let code = formData.get('file');

        this.showLoading(true);
        this.hideError();

        try {
            const endpoint = '/submit';
            
            const response = await fetch(endpoint, {
                method: 'POST',
                body: formData
            });

            const result = await response.json();
            
            // Check if the response contains an error (compilation errors come as {error: "..."})
            if (result.error) {
                throw new Error(result.error);
            }
            
            // For non-2xx responses, also check for error details
            if (!response.ok) {
                const errorText = result.error || result.err?.msg || `HTTP ${response.status}`;
                throw new Error(errorText);
            }

            this.showResults(result, code, ticks);

        } catch (error) {
            this.showError(error.message);
        } finally {
            this.showLoading(false);
        }
    }

    showLoading(show) {
        const runBtn = document.getElementById('run-btn');
        const btnText = runBtn.querySelector('.btn-text');
        const btnLoading = runBtn.querySelector('.btn-loading');

        if (show) {
            runBtn.disabled = true;
            btnText.style.display = 'none';
            btnLoading.style.display = 'inline';
        } else {
            runBtn.disabled = false;
            btnText.style.display = 'inline';
            btnLoading.style.display = 'none';
        }
    }

    showError(message) {
        const errorDiv = document.getElementById('error-message');
        errorDiv.textContent = message;
        errorDiv.style.display = 'block';
    }

    hideError() {
        const errorDiv = document.getElementById('error-message');
        errorDiv.style.display = 'none';
    }

    clearForm() {
        const ticksInput = document.getElementById('ticks');
        if (ticksInput) {
            ticksInput.value = '10';
        }
        this.updateLineNumbers();
        this.hideError();
    }

    showResults(result, originalCode, ticks) {
        // Save results to sessionStorage for results page
        sessionStorage.setItem('simulationResult', JSON.stringify(result));
        sessionStorage.setItem('originalCode', originalCode);
        sessionStorage.setItem('ticks', ticks);

        // Navigate to results page
        window.location.href = 'results.html';
    }
}

class ResultsPage {
    constructor() {
        this.result = null;
        this.originalCode = '';
        this.ticks = 0;
        this.error = null;
        this.errorContext = null;
        this.initializePage();
    }

    initializePage() {
        // Get data from sessionStorage
        const resultData = sessionStorage.getItem('simulationResult');
        const codeData = sessionStorage.getItem('originalCode');
        const ticksData = sessionStorage.getItem('ticks');

        if (!resultData) {
            this.showError('No data to display');
            return;
        }

        try {
            this.result = JSON.parse(resultData);
            this.originalCode = this.normalizeSourceCode(codeData, this.result?.source_code);
            this.ticks = parseInt(ticksData) || 0;

            // Handle both compilation errors (field: error) and runtime errors (field: err)
            this.error = this.result.err || this.result.error || null;
            this.errorContext = this.extractErrorContext();
        } catch (error) {
            this.showError('Error loading data');
            return;
        }

        this.renderResults();
    }

    renderResults() {
        const container = document.querySelector('.results-container');
        if (!container) return;

        container.innerHTML = `
            <div class="results-header">
                <div>
                    <h1>Simulation Results</h1>
                    <p>Number of ticks: ${this.ticks}</p>
                </div>
                <a href="/" class="back-btn">← Back</a>
            </div>

            <div class="original-code">
                <h2>Source Code:</h2>
                ${this.originalCode ? `<pre><code>${this.escapeHtml(this.originalCode)}</code></pre>` : '<p style="color: #666; font-style: italic;">No source code available</p>'}
            </div>

            <div class="simulation-steps" id="simulation-steps">
                ${this.renderSteps()}
            </div>

            ${this.error ? `
                <div class="error-section">
                    <h2>Simulation Error</h2>
                    <div class="error-message">
                        <strong>Error:</strong> ${this.escapeHtml(this.error.msg)}
                        ${this.error.detail ? this.renderErrorDetail(this.error.detail) : ''}
                        ${this.errorContext ? this.renderErrorContext(this.errorContext) : ''}
                    </div>
                </div>
            ` : ''}
        `;
        this.initializeStepHandlers();
    }

    renderSteps() {
        if (!this.result.steps || !Array.isArray(this.result.steps)) {
            return '<div class="error-message">No execution step data available</div>';
        }

        return this.result.steps.map((step, index) => this.renderStep(step, index)).join('');
    }

    renderStep(step, index) {
        const instruction = step.instruction || {};
        const registersBefore = step.old_registers || {};
        const registersAfter = step.new_registers || {};
        const pc = registersBefore.pc || registersAfter.pc || 'N/A';

        // Format instruction for display - use mnemonic if available, otherwise fallback to obj parsing
        const instructionText = this.formatInstructionDisplay(instruction);

        return `
            <div class="step" data-step="${index}">
                <div class="step-header">
                    <h3>Step ${index + 1}</h3>
                    <div style="display: flex; align-items: center; gap: 10px;">
                        <span class="step-number">PC: 0x${pc.toString(16)}</span>
                        <span class="expand-icon">▼</span>
                    </div>
                </div>
                <div class="step-content">
                    <div class="instruction">
                        <strong>Instruction:</strong> ${this.escapeHtml(instructionText)}
                        ${this.renderInstructionDetails(instruction)}
                    </div>
                    
                    <div class="registers-container">
                        <div class="registers-section registers-before">
                            <div class="registers-header">Registers before execution</div>
                            <div class="registers-content">
                                ${this.renderRegistersReal(registersBefore.storage || [], registersAfter.storage || [])}
                            </div>
                        </div>
                        
                        <div class="registers-section registers-after">
                            <div class="registers-header">Registers after execution</div>
                            <div class="registers-content">
                                ${this.renderRegistersReal(registersAfter.storage || [], registersBefore.storage || [])}
                            </div>
                        </div>
                    </div>

                    ${this.renderStepFlags(step.flags)}
                </div>
            </div>
        `;
    }

    renderInstructionDetails(instruction) {
        if (!instruction || typeof instruction !== 'object') {
            return '';
        }

        const mnemonic = instruction.mnemonic || '';
        const code = instruction.code || '';
        const obj = instruction.obj || null;

        if (!mnemonic && !code && !obj) {
            return '';
        }

        const rows = [];

        if (mnemonic) {
            rows.push(`
                <tr>
                    <th scope="row">Mnemonic</th>
                    <td>${this.escapeHtml(mnemonic)}</td>
                </tr>
            `);
        }

        if (code) {
            rows.push(`
                <tr>
                    <th scope="row">Machine Code</th>
                    <td><code>${this.escapeHtml(code)}</code></td>
                </tr>
            `);
        }

        if (obj) {
            rows.push(`
                <tr>
                    <th scope="row">Object</th>
                    <td><pre>${this.escapeHtml(JSON.stringify(obj, null, 2))}</pre></td>
                </tr>
            `);
        }

        if (rows.length === 0) {
            return '';
        }

        return `
            <div class="instruction-meta">
                <table>
                    <tbody>${rows.join('')}</tbody>
                </table>
            </div>
        `;
    }

    renderStepFlags(flags) {
        if (!Array.isArray(flags) || flags.length === 0) {
            return '';
        }

        const rows = flags.map((flag, index) => {
            const name = flag?.name || flag?.flag || flag?.type || `Flag ${index + 1}`;
            const instruction = flag?.instruction || flag;
            const mnemonic = flag?.mnemonic || instruction?.mnemonic || '';
            const code = flag?.code || instruction?.code || '';
            const obj = flag?.obj || instruction?.obj || null;

            return `
                <tr>
                    <td>${this.escapeHtml(name)}</td>
                    <td>${mnemonic ? this.escapeHtml(mnemonic) : '<span style="color: #666;">—</span>'}</td>
                    <td>${code ? `<code>${this.escapeHtml(code)}</code>` : '<span style="color: #666;">—</span>'}</td>
                    <td>${obj ? `<pre>${this.escapeHtml(JSON.stringify(obj, null, 2))}</pre>` : '<span style="color: #666;">—</span>'}</td>
                </tr>
            `;
        }).join('');

        return `
            <div class="step-flags">
                <h4>Flags</h4>
                <div class="flags-table-wrapper">
                    <table class="flags-table">
                        <thead>
                            <tr>
                                <th>Flag</th>
                                <th>Mnemonic</th>
                                <th>Machine Code</th>
                                <th>Object</th>
                            </tr>
                        </thead>
                        <tbody>${rows}</tbody>
                    </table>
                </div>
            </div>
        `;
    }

    renderRegisters(registers, compareRegisters = {}) {
        const registerNames = ['x0', 'x1', 'x2', 'x3', 'x4', 'x5', 'x6', 'x7', 
                              'x8', 'x9', 'x10', 'x11', 'x12', 'x13', 'x14', 'x15',
                              'x16', 'x17', 'x18', 'x19', 'x20', 'x21', 'x22', 'x23',
                              'x24', 'x25', 'x26', 'x27', 'x28', 'x29', 'x30', 'x31'];
        
        const abiNames = ['zero', 'ra', 'sp', 'gp', 'tp', 't0', 't1', 't2',
                         's0', 's1', 'a0', 'a1', 'a2', 'a3', 'a4', 'a5',
                         'a6', 'a7', 's2', 's3', 's4', 's5', 's6', 's7',
                         's8', 's9', 's10', 's11', 't3', 't4', 't5', 't6'];

        let html = '<div class="register-grid">';
        
        registerNames.forEach((reg, index) => {
            const value = registers[reg] || registers[abiNames[index]] || '0';
            const compareValue = compareRegisters[reg] || compareRegisters[abiNames[index]] || '0';
            const changed = value !== compareValue;
            
            html += `
                <div class="register-item ${changed ? 'register-changed' : ''}">
                    <span class="register-name">${reg} (${abiNames[index]})</span>
                    <span class="register-value">0x${parseInt(value).toString(16).padStart(8, '0')}</span>
                </div>
            `;
        });
        
        html += '</div>';
        return html;
    }

    renderRegistersReal(storage, compareStorage = []) {
        const registerNames = ['x0', 'x1', 'x2', 'x3', 'x4', 'x5', 'x6', 'x7', 
                              'x8', 'x9', 'x10', 'x11', 'x12', 'x13', 'x14', 'x15',
                              'x16', 'x17', 'x18', 'x19', 'x20', 'x21', 'x22', 'x23',
                              'x24', 'x25', 'x26', 'x27', 'x28', 'x29', 'x30', 'x31'];
        
        const abiNames = ['zero', 'ra', 'sp', 'gp', 'tp', 't0', 't1', 't2',
                         's0', 's1', 'a0', 'a1', 'a2', 'a3', 'a4', 'a5',
                         'a6', 'a7', 's2', 's3', 's4', 's5', 's6', 's7',
                         's8', 's9', 's10', 's11', 't3', 't4', 't5', 't6'];

        let html = '<div class="register-grid">';
        
        registerNames.forEach((reg, index) => {
            const value = storage[index] || 0;
            const compareValue = compareStorage[index] || 0;
            const changed = value !== compareValue;
            
            html += `
                <div class="register-item ${changed ? 'register-changed' : ''}">
                    <span class="register-name">${reg} (${abiNames[index]})</span>
                    <span class="register-value">0x${value.toString(16).padStart(8, '0')}</span>
                </div>
            `;
        });
        
        html += '</div>';
        return html;
    }

    renderMemoryChanges(memoryChanges) {
        if (!memoryChanges || Object.keys(memoryChanges).length === 0) {
            return '';
        }

        let html = '<div class="memory-changes"><h4>Memory changes:</h4><div class="memory-grid">';
        
        Object.entries(memoryChanges).forEach(([address, value]) => {
            html += `
                <div class="memory-item">
                    <span class="memory-address">0x${parseInt(address).toString(16).padStart(8, '0')}</span>
                    <span class="memory-value">0x${parseInt(value).toString(16).padStart(8, '0')}</span>
                </div>
            `;
        });
        
        html += '</div></div>';
        return html;
    }

    formatInstructionDisplay(instruction, fallback = 'No instruction (most likely ebreak)') {
        if (!instruction || typeof instruction !== 'object') {
            return fallback;
        }

        if (instruction.mnemonic) {
            return instruction.mnemonic;
        }

        if (instruction.obj && typeof instruction.obj === 'object') {
            const obj = instruction.obj;

            if (obj.Addi && Array.isArray(obj.Addi)) {
                const [rd, rs1, imm] = obj.Addi;
                return `addi x${rd}, x${rs1}, ${imm}`;
            }

            if (obj.Add && Array.isArray(obj.Add)) {
                const [rd, rs1, rs2] = obj.Add;
                return `add x${rd}, x${rs1}, x${rs2}`;
            }

            if (obj.Sub && Array.isArray(obj.Sub)) {
                const [rd, rs1, rs2] = obj.Sub;
                return `sub x${rd}, x${rs1}, x${rs2}`;
            }

            if (obj.And && Array.isArray(obj.And)) {
                const [rd, rs1, rs2] = obj.And;
                return `and x${rd}, x${rs1}, x${rs2}`;
            }

            if (obj.Or && Array.isArray(obj.Or)) {
                const [rd, rs1, rs2] = obj.Or;
                return `or x${rd}, x${rs1}, x${rs2}`;
            }

            if (obj.Xor && Array.isArray(obj.Xor)) {
                const [rd, rs1, rs2] = obj.Xor;
                return `xor x${rd}, x${rs1}, x${rs2}`;
            }

            for (const [name, operands] of Object.entries(obj)) {
                if (!Array.isArray(operands)) {
                    continue;
                }

                const lower = name.toLowerCase();

                if (lower.startsWith('l')) {
                    const [rd, rs1, imm] = operands;
                    return `${lower} x${rd}, ${imm}(x${rs1})`;
                }

                if (lower.startsWith('s')) {
                    const [rs1, rs2, imm] = operands;
                    return `${lower} x${rs2}, ${imm}(x${rs1})`;
                }
            }
        }

        return fallback;
    }

    renderErrorContext(context) {
        if (!context) {
            return '';
        }

        const instructionText = this.formatInstructionDisplay(context.instruction, 'Unknown instruction');
        const mnemonic = context.instruction?.mnemonic || '';
        const machineCode = context.instruction?.code || '';
        const instructionObj = context.instruction?.obj || null;
        const addressText = context.address === null || context.address === undefined
            ? 'Unknown'
            : this.formatPrimitiveValue(context.address);

        return `
            <div class="error-instruction" style="margin-top: 16px;">
                <h3>Failed Instruction</h3>
                <div class="error-instruction-meta" style="display: grid; gap: 4px; margin-bottom: 12px;">
                    <div><strong>Address:</strong> ${this.escapeHtml(addressText)}</div>
                    ${mnemonic ? `<div><strong>Mnemonic:</strong> ${this.escapeHtml(mnemonic)}</div>` : ''}
                    ${instructionText && instructionText !== mnemonic ? `<div><strong>Decoded:</strong> ${this.escapeHtml(instructionText)}</div>` : ''}
                    ${machineCode ? `<div><strong>Machine Code:</strong> <code>${this.escapeHtml(machineCode)}</code></div>` : ''}
                </div>
                ${instructionObj ? `
                    <details class="error-instruction-operands" style="margin-bottom: 12px;">
                        <summary>Operand breakdown</summary>
                        <pre>${this.escapeHtml(JSON.stringify(instructionObj, null, 2))}</pre>
                    </details>
                ` : ''}
                ${context.registers ? `
                    <div class="error-registers">
                        <h3 style="margin-bottom: 8px;">
                            Register Snapshot
                        </h3>
                        ${this.renderRegisterSnapshot(context.registers, context.compareRegisters)}
                    </div>
                ` : ''}
            </div>
        `.trim();
    }

    renderRegisterSnapshot(storage, compareStorage = null) {
        if (!Array.isArray(storage) || storage.length === 0) {
            return '<p style="color: #666; font-style: italic;">Register data unavailable</p>';
        }

        return `
            <div class="registers-section registers-snapshot">
                <div class="registers-content">
                    ${this.renderRegistersReal(storage, compareStorage || storage)}
                </div>
            </div>
        `.trim();
    }

    extractErrorContext() {
        if (!this.error) {
            return null;
        }

        const detail = this.error.detail;
        const context = {
            instruction: null,
            address: null,
            registers: null
        };

        if (detail && typeof detail === 'object') {
            const primaryDetail = detail.InstructionError || detail.FetchError || detail.ExecutionError || null;

            if (primaryDetail && typeof primaryDetail === 'object') {
                context.address = primaryDetail.instruction_address ?? primaryDetail.address ?? null;
                context.instruction = primaryDetail.instruction || primaryDetail.current_instruction || primaryDetail.last_instruction || null;

                const afterCandidate = primaryDetail.registers_after
                    || primaryDetail.registers
                    || primaryDetail.register_snapshot
                    || primaryDetail.machine_state?.registers
                    || primaryDetail.state?.registers
                    || null;

                const beforeCandidate = primaryDetail.registers_before
                    || primaryDetail.previous_registers
                    || primaryDetail.old_registers
                    || null;

                const normalized = this.normalizeRegisterStorage(afterCandidate);
                const normalizedCompare = this.normalizeRegisterStorage(beforeCandidate);

                if (normalized) {
                    context.registers = normalized;
                    context.registerSource = 'error-detail';
                    if (normalizedCompare) {
                        context.compareRegisters = normalizedCompare;
                    }
                }
            }
        }

        if ((context.address === null || context.address === undefined) && typeof this.error.msg === 'string') {
            const parsedAddress = this.extractAddressFromMessage(this.error.msg);
            if (parsedAddress !== null) {
                context.address = parsedAddress;
            }
        }

        if (!context.instruction && typeof this.error.msg === 'string') {
            context.instruction = this.extractInstructionFromMessage(this.error.msg);
        }

        if (!context.registers) {
            const fallback = this.extractRegistersFromSteps();
            if (fallback) {
                context.registers = fallback.registers;
                context.registerSource = fallback.source;
                if (fallback.compareRegisters) {
                    context.compareRegisters = fallback.compareRegisters;
                }
                if (context.address === null || context.address === undefined) {
                    context.address = fallback.address;
                }
            }
        }

        if (!context.instruction && !context.registers && (context.address === null || context.address === undefined)) {
            return null;
        }

        return context;
    }

    normalizeSourceCode(storedCode, resultSource) {
        const candidates = [storedCode, resultSource];

        for (const candidate of candidates) {
            if (typeof candidate === 'string' && candidate.toLowerCase() !== 'null' && candidate.toLowerCase() !== 'undefined') {
                return candidate;
            }
        }

        return '';
    }

    extractRegistersFromSteps() {
        if (!Array.isArray(this.result?.steps) || this.result.steps.length === 0) {
            return null;
        }

        const lastStep = this.result.steps[this.result.steps.length - 1];
        if (!lastStep) {
            return null;
        }

        const afterRegisters = this.normalizeRegisterStorage(lastStep.new_registers);
        const beforeRegisters = this.normalizeRegisterStorage(lastStep.old_registers);
        const registers = afterRegisters || beforeRegisters;
        if (!registers) {
            return null;
        }

        const address = (lastStep.new_registers && typeof lastStep.new_registers.pc === 'number')
            ? lastStep.new_registers.pc
            : (lastStep.old_registers && typeof lastStep.old_registers.pc === 'number')
                ? lastStep.old_registers.pc
                : null;

        return {
            registers,
            compareRegisters: afterRegisters && beforeRegisters ? beforeRegisters : null,
            address,
            source: 'last-step'
        };
    }

    normalizeRegisterStorage(candidate) {
        if (!candidate) {
            return null;
        }

        if (Array.isArray(candidate)) {
            return candidate;
        }

        if (Array.isArray(candidate.storage)) {
            return candidate.storage;
        }

        if (candidate.registers) {
            return this.normalizeRegisterStorage(candidate.registers);
        }

        return null;
    }

    extractAddressFromMessage(message) {
        if (typeof message !== 'string') {
            return null;
        }

        const match = message.match(/0x[0-9a-fA-F]+/);
        if (!match) {
            return null;
        }

        const parsed = Number.parseInt(match[0], 16);
        return Number.isFinite(parsed) ? parsed : null;
    }

    extractInstructionFromMessage(message) {
        if (typeof message !== 'string') {
            return null;
        }

        const match = message.match(/instruction:\s*([^:]+)/i);
        if (!match) {
            return null;
        }

        const mnemonic = match[1].trim();
        if (!mnemonic) {
            return null;
        }

        return { mnemonic };
    }

    initializeStepHandlers() {
        const stepHeaders = document.querySelectorAll('.step-header');
        
        stepHeaders.forEach(header => {
            header.addEventListener('click', () => {
                const step = header.parentElement;
                step.classList.toggle('expanded');
            });
        });

        // Auto-expand first step
        const firstStep = document.querySelector('.step');
        if (firstStep) {
            firstStep.classList.add('expanded');
        }
    }

    renderErrorDetail(detail) {
        if (detail === null || detail === undefined) {
            return '<br><strong>Details:</strong><pre>No additional detail provided</pre>';
        }

        const structured = this.formatErrorDetail(detail);
        const structuredSafe = this.escapeHtml(structured);
        const rawJson = this.escapeHtml(JSON.stringify(detail, null, 2));

        return `
            <br><strong>Details:</strong>
            <pre>${structuredSafe}</pre>
            <details class="raw-error">
                <summary>Raw error payload</summary>
                <pre>${rawJson}</pre>
            </details>
        `.trim();
    }

    formatErrorDetail(data, indent = 0) {
        const pad = ' '.repeat(indent);

        if (data === null) {
            return `${pad}null`;
        }

        if (data === undefined) {
            return `${pad}undefined`;
        }

        if (typeof data !== 'object') {
            return `${pad}${this.formatPrimitiveValue(data)}`;
        }

        if (Array.isArray(data)) {
            if (data.length === 0) {
                return `${pad}[]`;
            }

            return data.map((item, index) => {
                if (typeof item === 'object' && item !== null) {
                    return `${pad}[${index}]:\n${this.formatErrorDetail(item, indent + 2)}`;
                }
                return `${pad}[${index}]: ${this.formatPrimitiveValue(item)}`;
            }).join('\n');
        }

        const entries = Object.entries(data);

        if (entries.length === 0) {
            return `${pad}{}`;
        }

        return entries.map(([key, value]) => {
            if (typeof value === 'object' && value !== null) {
                return `${pad}${key}:\n${this.formatErrorDetail(value, indent + 2)}`;
            }
            return `${pad}${key}: ${this.formatPrimitiveValue(value)}`;
        }).join('\n');
    }

    formatPrimitiveValue(value) {
        if (typeof value === 'number' && Number.isFinite(value)) {
            const hex = value < 0 ? `-0x${Math.abs(value).toString(16)}` : `0x${value.toString(16)}`;
            return `${value} (${hex})`;
        }

        if (typeof value === 'string') {
            const numeric = Number(value);

            if (!Number.isNaN(numeric) && Number.isFinite(numeric)) {
                const hex = numeric < 0 ? `-0x${Math.abs(numeric).toString(16)}` : `0x${numeric.toString(16)}`;
                return `${value} (${hex})`;
            }

            return value;
        }

        if (typeof value === 'boolean') {
            return value ? 'true' : 'false';
        }

        if (value === null) {
            return 'null';
        }

        return JSON.stringify(value);
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    showError(message) {
        document.body.innerHTML = `
            <div class="container">
                <div class="results-container">
                    <div class="error-message">
                        <h2>Error</h2>
                        <p>${message}</p>
                        <a href="/" class="back-btn" style="margin-top: 20px; display: inline-block;">← Back to Home</a>
                    </div>
                </div>
            </div>
        `;
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    if (window.location.pathname.endsWith('results.html')) {
        new ResultsPage();
    } else {
        new RISCVSimulator();
    }
});