class RISCVSimulator {
    constructor() {
        this.initializeEventListeners();
        this.updateLineNumbers();
    }

    initializeEventListeners() {
        const form = document.getElementById('simulator-form');
        const codeTextarea = document.getElementById('code');
        const clearBtn = document.getElementById('clear-btn');

        form.addEventListener('submit', (e) => this.handleSubmit(e));
        codeTextarea.addEventListener('input', () => this.updateLineNumbers());
        codeTextarea.addEventListener('scroll', () => this.syncLineNumbers());
        codeTextarea.addEventListener('keydown', (e) => this.handleTabKey(e));
        clearBtn.addEventListener('click', () => this.clearForm());
    }

    updateLineNumbers() {
        const textarea = document.getElementById('code');
        const lineNumbers = document.querySelector('.line-numbers');
        const lines = textarea.value.split('\n').length;
        
        let numbers = '';
        for (let i = 1; i <= lines; i++) {
            numbers += i + '\n';
        }
        lineNumbers.textContent = numbers;
    }

    syncLineNumbers() {
        const textarea = document.getElementById('code');
        const lineNumbers = document.querySelector('.line-numbers');
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
        const code = formData.get('code');

        this.showLoading(true);
        this.hideError();

        try {
            const endpoint = '/submit';
            
            const response = await fetch(endpoint, {
                method: 'POST',
                body: formData
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || `HTTP ${response.status}`);
            }

            const result = await response.json();
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
        document.getElementById('code').value = '.global _start\n_start:\n    li a0, 10\n    li a1, 20\n    add a2, a0, a1\n    sub a3, a2, a0\n    addi a4, a1, 5';
        document.getElementById('ticks').value = '10';
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
            this.originalCode = codeData || '';
            this.ticks = parseInt(ticksData) || 0;

            // Save error but continue displaying steps
            this.error = this.result.err || null;
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
                <pre><code>${this.escapeHtml(this.originalCode)}</code></pre>
            </div>

            <div class="simulation-steps" id="simulation-steps">
                ${this.renderSteps()}
            </div>

            ${this.error ? `
                <div class="error-section">
                    <h2>Simulation Error</h2>
                    <div class="error-message">
                        <strong>Error:</strong> ${this.escapeHtml(this.error.msg)}
                        ${this.error.detail ? `<br><strong>Details:</strong> <pre>${this.escapeHtml(JSON.stringify(this.error.detail, null, 2))}</pre>` : ''}
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

        // Format instruction for display
        let instructionText = 'No instruction (most likely ebreak)';
        
        // Check all possible instruction types
        if (instruction.Addi && Array.isArray(instruction.Addi)) {
            const [rd, rs1, imm] = instruction.Addi;
            instructionText = `addi x${rd}, x${rs1}, ${imm}`;
        } else if (instruction.Add && Array.isArray(instruction.Add)) {
            const [rd, rs1, rs2] = instruction.Add;
            instructionText = `add x${rd}, x${rs1}, x${rs2}`;
        } else if (instruction.Sub && Array.isArray(instruction.Sub)) {
            const [rd, rs1, rs2] = instruction.Sub;
            instructionText = `sub x${rd}, x${rs1}, x${rs2}`;
        } else if (instruction.And && Array.isArray(instruction.And)) {
            const [rd, rs1, rs2] = instruction.And;
            instructionText = `and x${rd}, x${rs1}, x${rs2}`;
        } else if (instruction.Or && Array.isArray(instruction.Or)) {
            const [rd, rs1, rs2] = instruction.Or;
            instructionText = `or x${rd}, x${rs1}, x${rs2}`;
        } else if (instruction.Xor && Array.isArray(instruction.Xor)) {
            const [rd, rs1, rs2] = instruction.Xor;
            instructionText = `xor x${rd}, x${rs1}, x${rs2}`;
        }

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