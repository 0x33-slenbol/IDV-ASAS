'use strict';

// ======================== State ========================

let resources = null;
let charZhToStem = {};
let talentStemMap = {};
let selectedMap = '';
let selectedChars = [];
let charPage = 0;
let plans = [];
let currentPlanIdx = 0;
let roundUsage = null;
let roundCost = 0;
let roundEstimated = false;
let hasUnsavedSession = false;
let currentEvtSource = null;
let sessionRoundsCount = 0;

// ======================== Helpers ========================

function $(id) { return document.getElementById(id); }

function showStep(name) {
    document.querySelectorAll('.step').forEach(el => el.style.display = 'none');
    const el = $('step-' + name);
    if (el) el.style.display = 'flex';
}

async function api(path, method, body) {
    const opts = { method: method || 'GET' };
    if (body !== undefined) {
        opts.headers = { 'Content-Type': 'application/json' };
        opts.body = JSON.stringify(body);
    }
    const res = await fetch(path, opts);
    return res.json();
}

function loadImage(src) {
    return new Promise((resolve, reject) => {
        const img = new Image();
        img.onload = () => resolve(img);
        img.onerror = () => reject(new Error('Failed to load: ' + src));
        img.src = src;
    });
}

// ======================== Step 0: Config ========================

async function initConfig() {
    const cfg = await api('/api/config');
    if (cfg.has_default) {
        $('config-default-section').style.display = 'block';
        $('config-form-section').style.display = 'none';
        $('config-default-info').textContent =
            `检测到默认配置：\n  端点 ${cfg.base_url}  |  模型 ${cfg.model}  |  温度 ${cfg.temperature}`;
        $('config-default-price').textContent =
            `  输入单价 ¥${cfg.price_input_per_m.toFixed(4)}  |  输出单价 ¥${cfg.price_output_per_m.toFixed(4)} 每百万 tokens`;
    } else {
        showConfigForm();
    }
    showStep('config');
}

function showConfigForm() {
    $('config-default-section').style.display = 'none';
    $('config-form-section').style.display = 'block';
    const cfg = window._lastConfig || {};
    if (cfg.base_url !== undefined) $('cfg-base-url').value = cfg.base_url;
    if (cfg.model !== undefined) $('cfg-model').value = cfg.model;
    if (cfg.temperature !== undefined) $('cfg-temperature').value = cfg.temperature;
    if (cfg.price_input_per_m !== undefined) $('cfg-price-in').value = cfg.price_input_per_m;
    if (cfg.price_output_per_m !== undefined) $('cfg-price-out').value = cfg.price_output_per_m;
}

$('btn-config-use-default').addEventListener('click', async () => {
    await initHistory();
});

$('btn-config-edit').addEventListener('click', () => {
    showConfigForm();
});

$('btn-config-confirm').addEventListener('click', async () => {
    const maxTokensStr = $('cfg-max-tokens').value.trim();
    const body = {
        base_url: $('cfg-base-url').value,
        api_key: $('cfg-api-key').value,
        model: $('cfg-model').value,
        temperature: parseFloat($('cfg-temperature').value) || 0.7,
        max_tokens: maxTokensStr ? parseInt(maxTokensStr) : null,
        price_input_per_m: parseFloat($('cfg-price-in').value) || 0,
        price_output_per_m: parseFloat($('cfg-price-out').value) || 0,
        save_default: $('cfg-save-default').checked,
    };
    const res = await api('/api/config', 'POST', body);
    if (res.error) {
        alert(res.error);
        return;
    }
    window._lastConfig = {
        base_url: body.base_url,
        model: body.model,
        temperature: body.temperature,
        price_input_per_m: body.price_input_per_m,
        price_output_per_m: body.price_output_per_m,
    };
    await initHistory();
});

// ======================== History Import ========================

async function initHistory() {
    const list = await api('/api/history');
    if (!list || list.length === 0) {
        showStep('global-req');
        return;
    }
    showStep('history');
    $('history-list-section').style.display = 'block';
    const container = $('history-list');
    container.innerHTML = '';
    list.forEach(item => {
        const div = document.createElement('div');
        div.className = 'history-item';
        const summary = `模型 ${item.model}  |  ${item.rounds_count} 局  |  累计 ${item.prompt_tokens + item.completion_tokens} tokens  ¥${item.cost.toFixed(4)}  |  全局需求：${item.global_req || '（无）'}`;
        div.innerHTML = `<span class="history-name">${item.name}</span><span class="history-summary">${summary}</span>`;
        div.addEventListener('click', async () => {
            const res = await api('/api/history/load', 'POST', { name: item.name });
            if (res.error) {
                alert(res.error);
                return;
            }
            sessionRoundsCount = res.rounds_count;
            await loadResources();
            showStep('map-select');
            renderMapGrid();
            $('map-imported-info').style.display = 'block';
            $('map-imported-info').textContent = `✓ 已导入历史会话（共 ${res.rounds_count} 局记录）`;
        });
        container.appendChild(div);
    });
}

$('btn-history-yes').addEventListener('click', () => {
    $('history-list-section').style.display = 'block';
});

$('btn-history-no').addEventListener('click', () => {
    showStep('global-req');
});

// ======================== Step 1: Global Requirements ========================

$('btn-global-req-confirm').addEventListener('click', async () => {
    const text = $('global-req-input').value;
    await api('/api/session/global-req', 'POST', { text });
    await loadResources();
    showStep('map-select');
    renderMapGrid();
    $('map-imported-info').style.display = 'none';
});

// ======================== Resources Loading ========================

async function loadResources() {
    if (resources) return;
    resources = await api('/api/resources');
    resources.characters.forEach(c => { charZhToStem[c.zh] = c.stem; });
    talentStemMap = resources.talent_map || {};
}

// ======================== Step 2: Map Selection ========================

function renderMapGrid() {
    const grid = $('map-grid');
    grid.innerHTML = '';
    resources.maps.forEach(map => {
        const cell = document.createElement('div');
        cell.className = 'map-cell';
        if (selectedMap === map.stem) cell.classList.add('selected');
        cell.innerHTML = `<img src="/icons/maps/${map.stem}.png" alt="${map.zh}"><div class="map-name">${map.zh}</div>`;
        cell.addEventListener('click', () => {
            if (selectedMap === map.stem) {
                selectedMap = '';
            } else {
                selectedMap = map.stem;
            }
            renderMapGrid();
            $('btn-map-confirm').disabled = !selectedMap;
        });
        grid.appendChild(cell);
    });
    $('btn-map-confirm').disabled = !selectedMap;
}

$('btn-map-confirm').addEventListener('click', () => {
    if (!selectedMap) return;
    selectedChars = [];
    charPage = 0;
    showStep('char-select');
    renderCharGrid();
});

// ======================== Step 3: Character Selection ========================

function renderCharGrid() {
    const grid = $('char-grid');
    grid.innerHTML = '';
    const pageStart = charPage * 18;
    const full = selectedChars.length >= 4;

    for (let row = 0; row < 3; row++) {
        for (let col = 0; col < 6; col++) {
            let idx;
            if (col < 3) {
                idx = pageStart + row * 3 + col;
            } else {
                idx = pageStart + 9 + row * 3 + (col - 3);
            }

            if (idx >= resources.characters.length) {
                const empty = document.createElement('div');
                empty.className = 'char-cell';
                empty.style.visibility = 'hidden';
                grid.appendChild(empty);
                continue;
            }

            const char = resources.characters[idx];
            const cell = document.createElement('div');
            const isSel = selectedChars.includes(char.stem);
            const disabled = full && !isSel;
            cell.className = 'char-cell';
            if (isSel) cell.classList.add('selected');
            if (disabled) cell.classList.add('disabled');
            cell.innerHTML = `<img src="/icons/characters/${char.stem}.png" alt="${char.zh}"><div class="char-name">${char.zh}</div>`;
            if (!disabled) {
                cell.addEventListener('click', () => {
                    const pos = selectedChars.indexOf(char.stem);
                    if (pos >= 0) {
                        selectedChars.splice(pos, 1);
                    } else if (selectedChars.length < 4) {
                        selectedChars.push(char.stem);
                    }
                    renderCharGrid();
                    renderSelectedChars();
                });
            }
            grid.appendChild(cell);
        }
    }

    $('char-page-info').textContent = `第 ${charPage + 1} / 3 页`;
    $('btn-char-prev').disabled = charPage === 0;
    $('btn-char-next').disabled = charPage >= 2;
    $('btn-char-confirm').disabled = selectedChars.length !== 4;
}

function renderSelectedChars() {
    const container = $('selected-chars');
    container.innerHTML = '';
    for (let i = 0; i < 4; i++) {
        if (i < selectedChars.length) {
            const stem = selectedChars[i];
            const char = resources.characters.find(c => c.stem === stem);
            const zh = char ? char.zh : stem;
            const item = document.createElement('div');
            item.className = 'selected-char-item';
            item.innerHTML = `<img src="/icons/characters/${stem}.png" alt="${zh}"><span class="char-name">${zh}</span>`;
            container.appendChild(item);
        } else {
            const empty = document.createElement('div');
            empty.className = 'selected-char-empty';
            empty.textContent = '（空）';
            container.appendChild(empty);
        }
    }
}

$('btn-char-prev').addEventListener('click', () => {
    if (charPage > 0) { charPage--; renderCharGrid(); }
});

$('btn-char-next').addEventListener('click', () => {
    if (charPage < 2) { charPage++; renderCharGrid(); }
});

$('btn-char-confirm').addEventListener('click', () => {
    if (selectedChars.length !== 4) return;
    showStep('round-req');
    $('round-req-input').value = '';
});

// ======================== Step 4: Round Requirements ========================

$('btn-round-req-confirm').addEventListener('click', async () => {
    const roundReq = $('round-req-input').value;
    const res = await api('/api/session/round', 'POST', {
        map: selectedMap,
        characters: selectedChars,
        round_req: roundReq,
    });
    if (res.error) {
        alert(res.error);
        return;
    }
    startAnalysis();
});

// ======================== Step 5: Analyzing ========================

function startAnalysis() {
    showStep('analyzing');
    const cfg = window._lastConfig || {};
    $('analyzing-model-info').textContent = `模型：${cfg.model || ''}  @  ${cfg.base_url || ''}`;
    $('analyzing-retry-info').style.display = 'none';

    if (currentEvtSource) {
        currentEvtSource.close();
        currentEvtSource = null;
    }

    currentEvtSource = new EventSource('/api/session/analyze');

    currentEvtSource.onmessage = (e) => {
        let data;
        try {
            data = JSON.parse(e.data);
        } catch (err) {
            console.error('SSE parse error:', err);
            return;
        }

        if (data.type === 'retry') {
            $('analyzing-retry-info').style.display = 'block';
            $('analyzing-retry-info').textContent = `正在重试（第 ${data.count} 次）……`;
        } else if (data.type === 'plans') {
            currentEvtSource.close();
            currentEvtSource = null;
            plans = data.plans;
            currentPlanIdx = 0;
            roundUsage = data.usage;
            roundCost = data.cost;
            roundEstimated = data.estimated;
            hasUnsavedSession = true;
            showResultDisplay(false);
        } else if (data.type === 'error') {
            currentEvtSource.close();
            currentEvtSource = null;
            showErrorExit(data.message);
        }
    };

    currentEvtSource.onerror = () => {
        if (currentEvtSource) {
            currentEvtSource.close();
            currentEvtSource = null;
        }
        if (document.getElementById('step-analyzing').style.display !== 'none') {
            showErrorExit('与服务器的连接断开');
        }
    };
}

$('btn-interrupt').addEventListener('click', async () => {
    await api('/api/session/interrupt', 'POST');
});

// ======================== Step 6: Result Display ========================

async function showResultDisplay(locked) {
    if (!locked) {
        showStep('result');
        $('result-plan-title').textContent = '天赋选择';
        await renderPlan('map-canvas', 'talent-list', 'result-description', 'btn-plan-confirm', false);
        updatePlanPagination();
    } else {
        showStep('waiting-end');
        $('locked-plan-title').textContent = `方案 ${currentPlanIdx + 1}（已确认）`;
        await renderPlan('map-canvas-locked', 'locked-talent-list', 'locked-description', 'btn-end-round', true);
    }
}

async function renderPlan(canvasId, talentListId, descId, btnId, locked) {
    const plan = plans[currentPlanIdx];
    if (!plan) return;

    // Render canvas
    const canvas = $(canvasId);
    const posRes = await api(`/api/positions/${selectedMap}`);
    if (posRes.error) {
        console.error('Positions error:', posRes.error);
        return;
    }
    await composeMap(canvas, selectedMap, plan, posRes);

    // Render talents
    const talentList = $(talentListId);
    talentList.innerHTML = '';
    for (const sel of plan.selections) {
        const stem = charZhToStem[sel.character];
        const talentIcons = sel.talents.map(t => {
            const tStem = talentStemMap[t] || '';
            return tStem ? `<img src="/icons/personas/${tStem}.png" alt="${t}">` : '';
        }).join('');
        const charIcon = stem ? `<img class="talent-char-icon" src="/icons/characters/${stem}.png" alt="${sel.character}">` : '';
        const item = document.createElement('div');
        item.className = 'talent-item';
        item.innerHTML = `
            ${charIcon}
            <div class="talent-icons">${talentIcons}</div>
            <div class="talent-info">
                <span class="char-name">${sel.character}</span>
                <span class="talent-names">${sel.talents.join('、')}</span>
                <span class="point-info">${sel.point}号选点</span>
            </div>`;
        talentList.appendChild(item);
    }

    // Description
    $(descId).textContent = plan.description;

    // Button
    const btn = $(btnId);
    if (!locked) {
        btn.textContent = '确认使用';
        btn.onclick = () => {
            showResultDisplay(true);
        };
    } else {
        btn.textContent = '对局结束';
        btn.onclick = () => endRound();
    }
}

function updatePlanPagination() {
    $('plan-page-info').textContent = `${currentPlanIdx + 1} / ${plans.length}`;
    $('btn-plan-prev').disabled = currentPlanIdx === 0;
    $('btn-plan-next').disabled = currentPlanIdx + 1 >= plans.length;
}

$('btn-plan-prev').addEventListener('click', async () => {
    if (currentPlanIdx > 0) {
        currentPlanIdx--;
        await renderPlan('map-canvas', 'talent-list', 'result-description', 'btn-plan-confirm', false);
        updatePlanPagination();
    }
});

$('btn-plan-next').addEventListener('click', async () => {
    if (currentPlanIdx + 1 < plans.length) {
        currentPlanIdx++;
        await renderPlan('map-canvas', 'talent-list', 'result-description', 'btn-plan-confirm', false);
        updatePlanPagination();
    }
});

// ======================== Canvas Composition ========================

async function composeMap(canvas, mapStem, plan, positions) {
    const mapImg = await loadImage(`/icons/area-selection/${mapStem}.png`);

    const maxW = 700;
    const maxH = 550;
    const scale = Math.min(
        maxW / positions.map_width,
        maxH / positions.map_height
    );
    const displayW = Math.round(positions.map_width * scale);
    const displayH = Math.round(positions.map_height * scale);

    canvas.width = displayW;
    canvas.height = displayH;

    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, displayW, displayH);
    ctx.drawImage(mapImg, 0, 0, displayW, displayH);

    const iconSize = positions.icon_size * scale;

    for (const sel of plan.selections) {
        const stem = charZhToStem[sel.character];
        if (!stem) continue;
        const point = positions.points.find(p => p.num === sel.point);
        if (!point) continue;

        try {
            const iconImg = await loadImage(`/icons/characters/${stem}.png`);
            const cx = point.x * scale;
            const cy = point.y * scale;
            const half = iconSize / 2;
            ctx.drawImage(iconImg, cx - half, cy - half, iconSize, iconSize);
        } catch (e) {
            console.error(`Failed to load icon for ${sel.character}:`, e);
        }
    }
}

// ======================== Step 7: Waiting End ========================

async function endRound() {
    const res = await api('/api/session/end-round', 'POST');
    if (res.error) {
        alert(res.error);
        return;
    }
    sessionRoundsCount = res.session_totals.rounds_count;
    showAskNext(res);
}

// ======================== Step 8: Ask Next ========================

function showAskNext(endData) {
    showStep('ask-next');
    const info = $('round-usage-info');
    let html = '';
    if (endData.usage) {
        const u = endData.usage;
        const estSuffix = endData.estimated ? '（按字符估算）' : '';
        html += `<div class="subheading">本局 Token 用量：输入 ${u.prompt_tokens} + 输出 ${u.completion_tokens} = ${u.prompt_tokens + u.completion_tokens} tokens${estSuffix}</div>`;
        const cfg = window._lastConfig || {};
        const priceIn = cfg.price_input_per_m || 0;
        const priceOut = cfg.price_output_per_m || 0;
        html += `<div class="info-label">费用：${u.prompt_tokens} / 1M × ¥${priceIn.toFixed(4)} + ${u.completion_tokens} / 1M × ¥${priceOut.toFixed(4)} = ¥${endData.cost.toFixed(4)}</div>`;
    } else {
        html += `<div class="info-label">本局未产生 Token 用量。</div>`;
    }
    if (endData.session_totals) {
        const st = endData.session_totals;
        html += `<div class="info-label" style="margin-top:12px;">本次会话累计：输入 ${st.prompt_tokens} tokens，输出 ${st.completion_tokens} tokens，费用 ¥${st.cost.toFixed(4)}（共 ${st.rounds_count} 局）</div>`;
    }
    info.innerHTML = html;
}

$('btn-next-round').addEventListener('click', () => {
    selectedMap = '';
    selectedChars = [];
    charPage = 0;
    plans = [];
    currentPlanIdx = 0;
    roundUsage = null;
    roundCost = 0;
    roundEstimated = false;
    showStep('map-select');
    renderMapGrid();
    $('map-imported-info').style.display = 'none';
});

$('btn-exit').addEventListener('click', () => {
    showSaveHistory();
});

// ======================== Save History ========================

function showSaveHistory() {
    if (sessionRoundsCount === 0) {
        showExitMessage();
        return;
    }
    showStep('save-history');
    $('save-name-section').style.display = 'none';
    $('save-error').style.display = 'none';
    $('save-name-input').value = '';
}

$('btn-save-yes').addEventListener('click', () => {
    $('save-name-section').style.display = 'block';
});

$('btn-save-no').addEventListener('click', () => {
    hasUnsavedSession = false;
    showExitMessage();
});

$('btn-save-confirm').addEventListener('click', async () => {
    const name = $('save-name-input').value;
    const res = await api('/api/history/save', 'POST', { name });
    if (res.error) {
        $('save-error').textContent = `保存失败：${res.error}`;
        $('save-error').style.display = 'block';
        return;
    }
    hasUnsavedSession = false;
    showExitMessage();
});

$('btn-save-cancel').addEventListener('click', () => {
    $('save-name-section').style.display = 'none';
});

function showExitMessage() {
    const inner = document.querySelector('#step-save-history .step-inner');
    inner.innerHTML = `
        <h2 class="subheading">会话已结束</h2>
        <p class="info-label">感谢使用 IDV-ASAS，可以关闭浏览器。</p>
        <div class="btn-row">
            <button class="btn-yellow" id="btn-restart">重新开始</button>
        </div>`;
    $('btn-restart').addEventListener('click', () => location.reload());
}

// ======================== Error Exit ========================

function showErrorExit(message) {
    showStep('error-exit');
    $('error-msg').textContent = `错误：${message}`;
    const actions = $('error-actions');
    actions.innerHTML = '';

    const isModelErr = message.includes('不适配');
    const isInterrupted = message.includes('打断');

    if (isInterrupted || (!isModelErr && message !== 'API 线程异常断开')) {
        const row1 = document.createElement('div');
        row1.className = 'btn-row';
        const retryBtn = document.createElement('button');
        retryBtn.className = 'btn-yellow';
        retryBtn.textContent = '重试';
        retryBtn.addEventListener('click', () => {
            startAnalysis();
        });
        const reinputBtn = document.createElement('button');
        reinputBtn.className = 'btn-gray';
        reinputBtn.textContent = '重新输入地图与阵容';
        reinputBtn.addEventListener('click', () => {
            selectedMap = '';
            selectedChars = [];
            charPage = 0;
            showStep('map-select');
            renderMapGrid();
        });
        row1.appendChild(retryBtn);
        row1.appendChild(reinputBtn);
        actions.appendChild(row1);
    }

    const row2 = document.createElement('div');
    row2.className = 'btn-row';
    if (sessionRoundsCount > 0) {
        const saveBtn = document.createElement('button');
        saveBtn.className = 'btn-gray';
        saveBtn.textContent = '保存历史';
        saveBtn.addEventListener('click', () => showSaveHistory());
        row2.appendChild(saveBtn);
    }
    const exitBtn = document.createElement('button');
    exitBtn.className = 'btn-gray';
    exitBtn.textContent = '退出';
    exitBtn.addEventListener('click', () => {
        if (sessionRoundsCount > 0) {
            showSaveHistory();
        } else {
            showExitMessage();
        }
    });
    row2.appendChild(exitBtn);
    actions.appendChild(row2);
}

// ======================== Beforeunload ========================

window.addEventListener('beforeunload', (e) => {
    if (hasUnsavedSession) {
        e.preventDefault();
        e.returnValue = '';
    }
});

// ======================== Init ========================

(async function init() {
    try {
        const cfg = await api('/api/config');
        window._lastConfig = cfg;
        if (cfg.base_url) {
            window._lastConfig.price_input_per_m = cfg.price_input_per_m;
            window._lastConfig.price_output_per_m = cfg.price_output_per_m;
            window._lastConfig.model = cfg.model;
            window._lastConfig.base_url = cfg.base_url;
        }
        await loadResources();
        await initConfig();
    } catch (e) {
        console.error('Init error:', e);
    }
})();
