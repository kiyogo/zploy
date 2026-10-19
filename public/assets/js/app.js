// Zploy - Apple-inspired Minimalist Frontend Logic

let currentUser = null;
let isInitialized = true;
let selectedFiles = [];
let allSites = [];
let slugCheckTimeout = null;
let currentSlugValid = false;

// Active modal targets
let targetSiteForFiles = null;
let targetSiteForUpdate = null;
let targetSiteForReleases = null;
let targetSiteForEdit = null;
let targetSiteForDelete = null;

document.addEventListener('DOMContentLoaded', () => {
    initTheme();
    updateBaseUrlLabels();
    checkAuthStatus();
    initUploadEvents();
    initSlugEvents();
    if (window.location.hash === '#manage') {
        switchTab('manage');
    }
});

// ======================== THEME LOGIC ========================
function initTheme() {
    const savedTheme = localStorage.getItem('zploy_theme') || localStorage.getItem('minihost_theme') || 
        (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    setTheme(savedTheme);
}

function toggleTheme() {
    const current = document.documentElement.getAttribute('data-theme') || 'light';
    const next = current === 'dark' ? 'light' : 'dark';
    setTheme(next);
}

function setTheme(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('zploy_theme', theme);
    const moonIcon = document.getElementById('theme-icon-moon');
    const sunIcon = document.getElementById('theme-icon-sun');
    if (theme === 'dark') {
        moonIcon.style.display = 'none';
        sunIcon.style.display = 'block';
    } else {
        moonIcon.style.display = 'block';
        sunIcon.style.display = 'none';
    }
}

// ======================== API HELPER ========================
async function apiFetch(url, options = {}) {
    options.headers = options.headers || {};
    options.credentials = 'include';

    // Dual authentication: attach Bearer token if stored
    const token = localStorage.getItem('zploy_token');
    if (token) {
        options.headers['Authorization'] = `Bearer ${token}`;
    }

    // CSRF Header protection on state-modifying requests
    const method = (options.method || 'GET').toUpperCase();
    if (['POST', 'PUT', 'PATCH', 'DELETE'].includes(method)) {
        options.headers['X-Zploy-CSRF'] = '1';
        options.headers['X-MiniHost-CSRF'] = '1';
    }

    const res = await fetch(url, options);
    let data;
    try {
        data = await res.json();
    } catch {
        data = null;
    }

    if (!res.ok) {
        if (res.status === 401 && !url.includes('/api/auth/me')) {
            localStorage.removeItem('zploy_token');
            currentUser = null;
            window.location.href = '/admin/login';
            return;
        }
        const errorMsg = (data && data.error) || `Server error (${res.status})`;
        throw new Error(errorMsg);
    }
    return data;
}

// ======================== AUTH LOGIC ========================
async function checkAuthStatus() {
    try {
        const data = await apiFetch('/api/auth/me');
        isInitialized = data.initialized;
        currentUser = data.user;
        if (!currentUser) {
            localStorage.removeItem('zploy_token');
            window.location.href = '/admin/login';
            return;
        }
        renderAuthArea();

        if (!isInitialized) {
            window.location.href = '/admin/login';
            return;
        }
    } catch (err) {
        console.error('Auth check error:', err);
        window.location.href = '/admin/login';
    }
}

function renderAuthArea() {
    const authArea = document.getElementById('auth-area');
    if (currentUser) {
        authArea.innerHTML = `
            <div style="display: flex; align-items: center; gap: 8px;">
                <span style="font-size: 13px; font-weight: 500; color: var(--text-secondary);">${escapeHtml(currentUser.username)}</span>
                <button class="btn btn-ghost btn-sm" onclick="submitLogout()" title="Sign Out">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>
                        <polyline points="16 17 21 12 16 7"/>
                        <line x1="21" x2="9" y1="12" y2="12"/>
                    </svg>
                </button>
            </div>
        `;
    } else {
        authArea.innerHTML = `
            <button class="btn btn-secondary btn-sm" onclick="openLoginModal()">
                Sign In
            </button>
        `;
    }
}

function openLoginModal() {
    window.location.href = '/admin/login';
}

function closeLoginModal() {
    const modal = document.getElementById('login-modal');
    if (modal) modal.classList.remove('active');
}

async function submitLogin() {
    const username = document.getElementById('login-username').value.trim();
    const password = document.getElementById('login-password').value;
    const errorEl = document.getElementById('login-error-msg');
    const submitBtn = document.getElementById('btn-submit-login');

    if (!username || !password) {
        errorEl.textContent = 'Please enter both username and password';
        errorEl.style.display = 'block';
        return;
    }

    submitBtn.disabled = true;
    errorEl.style.display = 'none';

    try {
        const res = await apiFetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });

        if (res.token) {
            localStorage.setItem('zploy_token', res.token);
        }
        currentUser = res.user;
        renderAuthArea();
        closeLoginModal();
        showToast('Signed in successfully!', 'success');
        
        // If on manage view, reload sites
        if (document.getElementById('view-manage').style.display === 'block') {
            loadSitesList();
        }
    } catch (err) {
        errorEl.textContent = err.message;
        errorEl.style.display = 'block';
    } finally {
        submitBtn.disabled = false;
    }
}

async function submitLogout() {
    try {
        await apiFetch('/api/auth/logout', { method: 'POST' });
    } catch (err) {
        console.error(err);
    }
    localStorage.removeItem('zploy_token');
    currentUser = null;
    window.location.href = '/admin/login';
}

function openSetupModal() {
    document.getElementById('setup-modal').classList.add('active');
}

async function submitSetup() {
    const username = document.getElementById('setup-username').value.trim();
    const password = document.getElementById('setup-password').value;
    const errorEl = document.getElementById('setup-error-msg');
    const submitBtn = document.getElementById('btn-submit-setup');

    if (username.length < 3 || password.length < 6) {
        errorEl.textContent = 'Username must be at least 3 characters, password at least 6 characters';
        errorEl.style.display = 'block';
        return;
    }

    submitBtn.disabled = true;
    errorEl.style.display = 'none';

    try {
        await apiFetch('/api/auth/setup', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });

        document.getElementById('setup-modal').classList.remove('active');
        showToast('Admin account initialized successfully! Signing in...', 'success');

        // Auto login
        const loginRes = await apiFetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });
        if (loginRes.token) {
            localStorage.setItem('zploy_token', loginRes.token);
        }
        currentUser = loginRes.user;
        renderAuthArea();
    } catch (err) {
        errorEl.textContent = err.message;
        errorEl.style.display = 'block';
    } finally {
        submitBtn.disabled = false;
    }
}

// ======================== TABS SWITCHING ========================
function switchTab(tab) {
    const publishBtn = document.getElementById('tab-btn-publish');
    const manageBtn = document.getElementById('tab-btn-manage');
    const publishView = document.getElementById('view-publish');
    const manageView = document.getElementById('view-manage');

    if (tab === 'publish') {
        publishBtn.classList.add('active');
        manageBtn.classList.remove('active');
        publishView.style.display = 'block';
        manageView.style.display = 'none';
        history.replaceState(null, '', '/admin#publish');
    } else {
        manageBtn.classList.add('active');
        publishBtn.classList.remove('active');
        publishView.style.display = 'none';
        manageView.style.display = 'block';
        loadSitesList();
        history.replaceState(null, '', '/admin#manage');
    }
}

function updateBaseUrlLabels() {
    const origin = window.location.origin;
    const label = document.getElementById('slug-prefix-label');
    if (label) {
        label.textContent = `${origin}/`;
    }
}

// ======================== FILE UPLOAD & INSPECTION ========================
function initUploadEvents() {
    const dropZone = document.getElementById('drop-zone');
    const fileInput = document.getElementById('file-input');

    ['dragenter', 'dragover'].forEach(eventName => {
        dropZone.addEventListener(eventName, (e) => {
            e.preventDefault();
            e.stopPropagation();
            dropZone.classList.add('drag-over');
        });
    });

    ['dragleave', 'drop'].forEach(eventName => {
        dropZone.addEventListener(eventName, (e) => {
            e.preventDefault();
            e.stopPropagation();
            dropZone.classList.remove('drag-over');
        });
    });

    dropZone.addEventListener('drop', (e) => {
        const dt = e.dataTransfer;
        if (dt.files && dt.files.length > 0) {
            handleSelectedFiles(Array.from(dt.files));
        }
    });

    fileInput.addEventListener('change', (e) => {
        if (e.target.files && e.target.files.length > 0) {
            handleSelectedFiles(Array.from(e.target.files));
        }
    });

    // Update modal drop zone
    const updateDropZone = document.getElementById('update-drop-zone');
    const updateFileInput = document.getElementById('update-file-input');
    if (updateDropZone && updateFileInput) {
        updateFileInput.addEventListener('change', (e) => {
            if (e.target.files && e.target.files.length > 0) {
                const count = e.target.files.length;
                const firstName = e.target.files[0].name;
                const text = count === 1 ? firstName : `${firstName} and ${count - 1} other files`;
                document.getElementById('update-file-preview').textContent = `Selected: ${text}`;
            }
        });
    }
}

function handleSelectedFiles(files) {
    selectedFiles = files;
    renderFilePreview();
    inspectClientSideFiles(files);
}

function renderFilePreview() {
    const container = document.getElementById('file-preview-list');
    if (selectedFiles.length === 0) {
        container.style.display = 'none';
        container.innerHTML = '';
        return;
    }

    container.style.display = 'flex';
    container.innerHTML = '';

    // If a single zip
    if (selectedFiles.length === 1 && selectedFiles[0].name.toLowerCase().endsWith('.zip')) {
        const file = selectedFiles[0];
        container.innerHTML = `
            <div class="file-preview-item">
                <div class="file-info">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color: var(--accent);">
                        <path d="M10 2v20"/>
                        <path d="M14 2v20"/>
                        <rect x="4" y="2" width="16" height="20" rx="2"/>
                    </svg>
                    <div>
                        <div class="file-name">${escapeHtml(file.name)}</div>
                        <div class="file-size">${formatBytes(file.size)} | Website ZIP Archive</div>
                    </div>
                </div>
                <button class="btn btn-ghost btn-sm" onclick="clearSelectedFiles()">Remove</button>
            </div>
        `;

        // Suggest slug if empty
        const slugInput = document.getElementById('slug-input');
        if (!slugInput.value.trim()) {
            const suggested = file.name.replace(/\.zip$/i, '').toLowerCase().replace(/[^a-z0-9\-_]/g, '-');
            slugInput.value = suggested;
            validateSlugInput();
        }
    } else {
        // Multiple or single html
        const totalBytes = selectedFiles.reduce((acc, f) => acc + f.size, 0);
        container.innerHTML = `
            <div class="file-preview-item">
                <div class="file-info">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color: var(--accent);">
                        <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/>
                        <polyline points="14 2 14 8 20 8"/>
                    </svg>
                    <div>
                        <div class="file-name">${selectedFiles.length} files selected</div>
                        <div class="file-size">Total size: ${formatBytes(totalBytes)}</div>
                    </div>
                </div>
                <button class="btn btn-ghost btn-sm" onclick="clearSelectedFiles()">Remove all</button>
            </div>
        `;
    }
}

function clearSelectedFiles() {
    selectedFiles = [];
    document.getElementById('file-input').value = '';
    renderFilePreview();
    document.getElementById('path-warning-box').style.display = 'none';
}

// Client-side inspection of files to warn about root-absolute paths or missing index.html
function inspectClientSideFiles(files) {
    const warningBox = document.getElementById('path-warning-box');
    const warningText = document.getElementById('path-warning-text');
    warningBox.style.display = 'none';

    const isZip = files.length === 1 && files[0].name.toLowerCase().endsWith('.zip');
    if (isZip) {
        return; // Zips are inspected securely on server
    }

    let hasIndex = false;
    let htmlFile = null;

    for (const f of files) {
        const nameLower = f.name.toLowerCase();
        if (nameLower === 'index.html' || nameLower === 'index.htm') {
            hasIndex = true;
            htmlFile = f;
        } else if (nameLower.endsWith('.html') || nameLower.endsWith('.htm')) {
            htmlFile = f;
        }
    }

    if (files.length > 1 && !hasIndex) {
        warningBox.style.display = 'flex';
        warningText.textContent = "No 'index.html' found at the root. Zploy requires index.html to serve the main page.";
        return;
    }

    // Inspect HTML for absolute root paths: href="/..." or src="/..."
    if (htmlFile && htmlFile.size < 1024 * 1024) { // Only read under 1MB for client performance
        const reader = new FileReader();
        reader.onload = (e) => {
            const content = e.target.result;
            const absolutePathRegex = /(?:href|src)=["']\/[a-zA-Z0-9_\-]/i;
            if (absolutePathRegex.test(content)) {
                warningBox.style.display = 'flex';
                warningText.innerHTML = "<strong>Compatibility Tip:</strong> Your HTML uses root-relative paths like <code>/style.css</code> or <code>/assets/...</code>. Consider using relative paths like <code>./style.css</code> so assets load properly under subpaths.";
            }
        };
        reader.readAsText(htmlFile);
    }
}

// ======================== SLUG VALIDATION ========================
function initSlugEvents() {
    const slugInput = document.getElementById('slug-input');
    slugInput.addEventListener('input', () => {
        clearTimeout(slugCheckTimeout);
        slugCheckTimeout = setTimeout(validateSlugInput, 250);
    });
}

async function validateSlugInput() {
    const slugInput = document.getElementById('slug-input');
    const statusText = document.getElementById('slug-status-text');
    const rawVal = slugInput.value.trim();

    if (!rawVal) {
        statusText.innerHTML = '';
        statusText.className = 'slug-status';
        currentSlugValid = true;
        return;
    }

    statusText.className = 'slug-status checking';
    statusText.innerHTML = `
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="spin">
            <line x1="12" y1="2" x2="12" y2="6"/>
            <line x1="12" y1="18" x2="12" y2="22"/>
            <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"/>
            <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"/>
            <line x1="2" y1="12" x2="6" y2="12"/>
            <line x1="18" y1="12" x2="22" y2="12"/>
            <line x1="4.93" y1="19.07" x2="7.76" y2="16.24"/>
            <line x1="16.24" y1="7.76" x2="19.07" y2="4.93"/>
        </svg>
        <span>Checking availability...</span>
    `;

    try {
        const querySlug = rawVal === '/' ? '' : rawVal;
        const res = await apiFetch(`/api/slugs/check?slug=${encodeURIComponent(querySlug)}`);
        if (res.valid && res.available) {
            statusText.className = 'slug-status valid';
            statusText.innerHTML = `
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="20 6 9 17 4 12"/>
                </svg>
                <span>${escapeHtml(res.message)}</span>
            `;
            currentSlugValid = true;
        } else {
            statusText.className = 'slug-status invalid';
            statusText.innerHTML = `
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10"/>
                    <line x1="15" y1="9" x2="9" y2="15"/>
                    <line x1="9" y1="9" x2="15" y2="15"/>
                </svg>
                <span>${escapeHtml(res.message)}</span>
            `;
            currentSlugValid = false;
        }
    } catch {
        statusText.innerHTML = '';
        currentSlugValid = false;
    }
}

// ======================== PUBLISHING FLOW ========================
function startPublishing() {
    if (!currentUser) {
        showToast('Please sign in as admin to publish websites.', 'info');
        openLoginModal();
        return;
    }

    if (selectedFiles.length === 0) {
        showToast('Please select or drop website files to publish', 'warning');
        return;
    }

    const rawSlug = document.getElementById('slug-input').value.trim();
    // Normalize slug (empty or "/" becomes "")
    const slug = rawSlug === '/' ? '' : rawSlug.replace(/^\/+|\/+$/g, '');

    if (rawSlug && !currentSlugValid) {
        showToast('Invalid or already taken path. Please choose another.', 'warning');
        return;
    }

    const defaultName = slug ? slug : 'Root Website';
    const siteName = document.getElementById('site-name-input').value.trim() || defaultName;

    const formData = new FormData();
    formData.append('slug', slug);
    formData.append('name', siteName);

    selectedFiles.forEach(file => {
        formData.append('file', file);
    });

    const progressContainer = document.getElementById('upload-progress-container');
    const progressBar = document.getElementById('upload-progress-bar');
    const statusLabel = document.getElementById('upload-status-label');
    const percentLabel = document.getElementById('upload-percentage-label');
    const publishBtn = document.getElementById('btn-publish');

    progressContainer.style.display = 'block';
    publishBtn.disabled = true;
    statusLabel.textContent = 'Uploading...';
    progressBar.style.width = '0%';
    percentLabel.textContent = '0%';

    const xhr = new XMLHttpRequest();
    xhr.open('POST', '/api/uploads/quick-publish');
    xhr.setRequestHeader('X-Zploy-CSRF', '1');
    xhr.setRequestHeader('X-MiniHost-CSRF', '1');
    const token = localStorage.getItem('zploy_token');
    if (token) {
        xhr.setRequestHeader('Authorization', `Bearer ${token}`);
    }
    xhr.withCredentials = true;

    xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) {
            const percent = Math.round((e.loaded / e.total) * 100);
            progressBar.style.width = `${percent}%`;
            percentLabel.textContent = `${percent}%`;
            if (percent >= 100) {
                statusLabel.textContent = 'Extracting & deploying website...';
            }
        }
    };

    xhr.onload = () => {
        publishBtn.disabled = false;
        progressContainer.style.display = 'none';

        if (xhr.status >= 200 && xhr.status < 300) {
            try {
                const res = JSON.parse(xhr.responseText);
                onPublishSuccess(res);
            } catch {
                showToast('Failed to parse server response', 'danger');
            }
        } else {
            let errorMsg = 'Publishing failed';
            try {
                const errData = JSON.parse(xhr.responseText);
                errorMsg = errData.error || errorMsg;
            } catch {}
            if (xhr.status === 401) {
                localStorage.removeItem('zploy_token');
                currentUser = null;
                renderAuthArea();
            }
            showToast(errorMsg, 'danger');
        }
    };

    xhr.onerror = () => {
        publishBtn.disabled = false;
        progressContainer.style.display = 'none';
        if (xhr.status === 401) {
            localStorage.removeItem('zploy_token');
            currentUser = null;
            renderAuthArea();
            showToast('Session has expired. Please sign in again.', 'danger');
        } else {
            showToast('Connection to server lost during upload. Please check server status.', 'danger');
        }
    };

    xhr.send(formData);
}

function onPublishSuccess(res) {
    document.getElementById('upload-card-container').style.display = 'none';
    const successCard = document.getElementById('publish-success-card');
    successCard.style.display = 'block';

    const fullUrl = res.url || (res.slug ? `${window.location.origin}/${res.slug}/` : `${window.location.origin}/`);
    const linkEl = document.getElementById('success-site-link');
    linkEl.href = fullUrl;
    linkEl.textContent = fullUrl;

    const openBtn = document.getElementById('btn-open-site');
    openBtn.href = fullUrl;

    showToast('Website published successfully!', 'success');
}

function copyPublishedUrl() {
    const linkEl = document.getElementById('success-site-link');
    navigator.clipboard.writeText(linkEl.href).then(() => {
        const copyBtn = document.getElementById('btn-copy-url');
        copyBtn.textContent = 'Copied!';
        setTimeout(() => copyBtn.textContent = 'Copy URL', 2000);
        showToast('URL copied to clipboard', 'info');
    });
}

function resetPublishForm() {
    clearSelectedFiles();
    document.getElementById('slug-input').value = '';
    document.getElementById('site-name-input').value = '';
    document.getElementById('slug-status-text').innerHTML = '';
    currentSlugValid = true;
    document.getElementById('publish-success-card').style.display = 'none';
    document.getElementById('upload-card-container').style.display = 'block';
}

// ======================== SITES MANAGEMENT ========================
async function loadSitesList() {
    try {
        const data = await apiFetch('/api/sites');
        allSites = data.sites || [];
        renderSitesGrid(allSites);
    } catch (err) {
        showToast(err.message, 'danger');
    }
}

function filterSitesList() {
    const query = document.getElementById('manage-search-input').value.trim().toLowerCase();
    if (!query) {
        renderSitesGrid(allSites);
        return;
    }
    const filtered = allSites.filter(s => 
        s.name.toLowerCase().includes(query) || s.slug.toLowerCase().includes(query)
    );
    renderSitesGrid(filtered);
}

function renderSitesGrid(sites) {
    const grid = document.getElementById('sites-grid');
    const emptyState = document.getElementById('sites-empty-state');

    if (sites.length === 0) {
        grid.innerHTML = '';
        emptyState.style.display = 'block';
        return;
    }

    emptyState.style.display = 'none';
    const origin = window.location.origin;

    grid.innerHTML = sites.map(site => {
        const fullUrl = site.slug ? `${origin}/${site.slug}/` : `${origin}/`;
        const slugLabel = site.slug ? `/${escapeHtml(site.slug)}` : `/ (Root)`;
        const updatedDate = formatDate(site.updated_at);
        const sizeFormatted = formatBytes(site.total_size_bytes);

        return `
            <div class="site-card" id="site-card-${escapeHtml(site.id)}">
                <div class="site-card-top">
                    <div>
                        <h4 class="site-card-name">${escapeHtml(site.name)}</h4>
                        <a href="${fullUrl}" target="_blank" class="site-card-slug">${slugLabel}</a>
                    </div>
                    <span class="badge badge-active">Active</span>
                </div>

                <div class="site-card-meta">
                    <span>${site.file_count} files</span>
                    <span>${sizeFormatted}</span>
                    <span>Updated: ${updatedDate}</span>
                </div>

                <div class="site-card-actions">
                    <div class="site-card-actions-primary">
                        <a href="${fullUrl}" target="_blank" class="btn btn-primary btn-sm" title="Open Website">
                            Open Site
                        </a>
                        <button class="btn btn-secondary btn-sm" onclick="copyToClipboard('${fullUrl}')" title="Copy URL">
                            Copy Link
                        </button>
                        <button class="btn btn-secondary btn-sm" onclick="openUpdateModal('${escapeHtml(site.id)}', '${escapeHtml(site.name)}')" title="Deploy new release">
                            Update
                        </button>
                    </div>
                    <div class="site-card-actions-secondary">
                        <button class="btn btn-ghost btn-xs" onclick="openFilesModal('${escapeHtml(site.id)}', '${escapeHtml(site.name)}')" title="View files">
                            Files
                        </button>
                        <button class="btn btn-ghost btn-xs" onclick="downloadSiteZip('${escapeHtml(site.id)}')" title="Download ZIP">
                            Download
                        </button>
                        <button class="btn btn-ghost btn-xs" onclick="openReleasesModal('${escapeHtml(site.id)}', '${escapeHtml(site.name)}')" title="Release history">
                            History
                        </button>
                        <button class="btn btn-ghost btn-xs" onclick="openEditModal('${escapeHtml(site.id)}', '${escapeHtml(site.name)}', '${escapeHtml(site.slug)}', ${site.spa_fallback})" title="Edit details">
                            Edit
                        </button>
                        <button class="btn btn-danger-subtle btn-xs" onclick="openDeleteModal('${escapeHtml(site.id)}', '${escapeHtml(site.name)}')" title="Delete website">
                            Delete
                        </button>
                    </div>
                </div>
            </div>
        `;
    }).join('');
}

// 1. Files Modal
async function openFilesModal(siteId, siteName) {
    targetSiteForFiles = siteId;
    document.getElementById('files-modal-title').textContent = `Files for: ${siteName}`;
    const body = document.getElementById('files-modal-body');
    body.innerHTML = '<div style="text-align:center;padding:20px;color:var(--text-secondary);">Loading files...</div>';
    document.getElementById('files-modal').classList.add('active');

    try {
        const data = await apiFetch(`/api/sites/${siteId}/files`);
        if (!data.files || data.files.length === 0) {
            body.innerHTML = '<div style="text-align:center;padding:20px;color:var(--text-secondary);">No files found.</div>';
            return;
        }

        body.innerHTML = data.files.map(f => `
            <div class="file-tree-item">
                <span style="font-family: monospace;">${escapeHtml(f.path)}</span>
                <span style="color: var(--text-tertiary);">${formatBytes(f.size_bytes)}</span>
            </div>
        `).join('');
    } catch (err) {
        body.innerHTML = `<div style="color:var(--danger);padding:10px;">${escapeHtml(err.message)}</div>`;
    }
}

function closeFilesModal() {
    document.getElementById('files-modal').classList.remove('active');
}

// 2. Download ZIP
function downloadSiteZip(siteId) {
    window.location.href = `/api/sites/${siteId}/download`;
    showToast('Generating and downloading ZIP...', 'info');
}

// 3. Update Modal
function openUpdateModal(siteId, siteName) {
    if (!currentUser) {
        showToast('Please sign in to update website', 'warning');
        openLoginModal();
        return;
    }
    targetSiteForUpdate = siteId;
    document.getElementById('update-modal-title').textContent = `Update: ${siteName}`;
    document.getElementById('update-file-input').value = '';
    document.getElementById('update-file-preview').textContent = '';
    document.getElementById('update-note-input').value = '';
    document.getElementById('update-error-msg').style.display = 'none';
    document.getElementById('update-modal').classList.add('active');
}

function closeUpdateModal() {
    document.getElementById('update-modal').classList.remove('active');
}

async function submitSiteUpdate() {
    const fileInput = document.getElementById('update-file-input');
    const noteInput = document.getElementById('update-note-input');
    const errorEl = document.getElementById('update-error-msg');
    const submitBtn = document.getElementById('btn-submit-update');

    if (!fileInput.files || fileInput.files.length === 0) {
        errorEl.textContent = 'Please select files for the update';
        errorEl.style.display = 'block';
        return;
    }

    submitBtn.disabled = true;
    errorEl.style.display = 'none';

    const formData = new FormData();
    if (noteInput.value.trim()) {
        formData.append('note', noteInput.value.trim());
    }

    Array.from(fileInput.files).forEach(f => {
        formData.append('file', f);
    });

    try {
        await apiFetch(`/api/sites/${targetSiteForUpdate}/releases`, {
            method: 'POST',
            body: formData
        });

        closeUpdateModal();
        showToast('Update deployed successfully!', 'success');
        loadSitesList();
    } catch (err) {
        errorEl.textContent = err.message;
        errorEl.style.display = 'block';
    } finally {
        submitBtn.disabled = false;
    }
}

// 4. Releases & Rollback Modal
async function openReleasesModal(siteId, siteName) {
    targetSiteForReleases = siteId;
    document.getElementById('releases-modal-title').textContent = `Release History: ${siteName}`;
    const body = document.getElementById('releases-modal-body');
    body.innerHTML = '<div style="text-align:center;padding:20px;">Loading history...</div>';
    document.getElementById('releases-modal').classList.add('active');

    try {
        const data = await apiFetch(`/api/sites/${siteId}/releases`);
        const siteData = await apiFetch(`/api/sites/${siteId}`);
        const activeReleaseId = siteData.site.active_release_id;

        if (!data.releases || data.releases.length === 0) {
            body.innerHTML = '<div style="text-align:center;padding:20px;">No releases found.</div>';
            return;
        }

        body.innerHTML = data.releases.map(rel => {
            const isActive = rel.id === activeReleaseId;
            return `
                <div style="display:flex;align-items:center;justify-content:space-between;padding:12px;border-bottom:1px solid var(--border-subtle);">
                    <div>
                        <div style="display:flex;align-items:center;gap:8px;">
                            <strong>Release v${rel.version_num}</strong>
                            ${isActive ? '<span class="badge badge-active">Active</span>' : ''}
                        </div>
                        <div style="font-size:12px;color:var(--text-tertiary);margin-top:4px;">
                            ${rel.file_count} files | ${formatBytes(rel.total_size_bytes)} | ${formatDate(rel.created_at)}
                        </div>
                        <div style="font-size:13px;color:var(--text-secondary);margin-top:2px;">
                            ${escapeHtml(rel.note || 'No release notes')}
                        </div>
                    </div>
                    <div>
                        ${!isActive && currentUser ? `
                            <button class="btn btn-secondary btn-sm" onclick="submitRollback('${siteId}', '${rel.id}')">
                                Rollback to this release
                            </button>
                        ` : ''}
                    </div>
                </div>
            `;
        }).join('');
    } catch (err) {
        body.innerHTML = `<div style="color:var(--danger);padding:10px;">${escapeHtml(err.message)}</div>`;
    }
}

function closeReleasesModal() {
    document.getElementById('releases-modal').classList.remove('active');
}

async function submitRollback(siteId, targetReleaseId) {
    if (!confirm('Are you sure you want to rollback to this release?')) return;
    try {
        await apiFetch(`/api/sites/${siteId}/rollback`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ target_release_id: targetReleaseId })
        });
        showToast('Rollback completed successfully!', 'success');
        closeReleasesModal();
        loadSitesList();
    } catch (err) {
        showToast(err.message, 'danger');
    }
}

// 5. Edit Site Modal
function openEditModal(siteId, name, slug, spaFallback) {
    if (!currentUser) {
        showToast('Please sign in to edit website details', 'warning');
        openLoginModal();
        return;
    }
    targetSiteForEdit = siteId;
    document.getElementById('edit-name-input').value = name;
    document.getElementById('edit-slug-input').value = slug;
    document.getElementById('edit-spa-checkbox').checked = spaFallback === 1;
    document.getElementById('edit-error-msg').style.display = 'none';
    document.getElementById('edit-modal').classList.add('active');
}

function closeEditModal() {
    document.getElementById('edit-modal').classList.remove('active');
}

async function submitSiteEdit() {
    const name = document.getElementById('edit-name-input').value.trim();
    const rawSlug = document.getElementById('edit-slug-input').value.trim();
    const slug = rawSlug === '/' ? '' : rawSlug.replace(/^\/+|\/+$/g, '');
    const spaFallback = document.getElementById('edit-spa-checkbox').checked;
    const errorEl = document.getElementById('edit-error-msg');
    const submitBtn = document.getElementById('btn-submit-edit');

    if (!name) {
        errorEl.textContent = 'Website name cannot be empty';
        errorEl.style.display = 'block';
        return;
    }

    submitBtn.disabled = true;
    errorEl.style.display = 'none';

    try {
        await apiFetch(`/api/sites/${targetSiteForEdit}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, slug, spa_fallback: spaFallback })
        });

        closeEditModal();
        showToast('Website details updated successfully!', 'success');
        loadSitesList();
    } catch (err) {
        errorEl.textContent = err.message;
        errorEl.style.display = 'block';
    } finally {
        submitBtn.disabled = false;
    }
}

// 6. Delete Site Modal
function openDeleteModal(siteId, name) {
    if (!currentUser) {
        showToast('Please sign in to delete website', 'warning');
        openLoginModal();
        return;
    }
    targetSiteForDelete = siteId;
    document.getElementById('delete-site-name').textContent = name;
    document.getElementById('delete-modal').classList.add('active');
}

function closeDeleteModal() {
    document.getElementById('delete-modal').classList.remove('active');
}

async function submitSiteDelete() {
    const submitBtn = document.getElementById('btn-confirm-delete');
    submitBtn.disabled = true;

    try {
        await apiFetch(`/api/sites/${targetSiteForDelete}`, {
            method: 'DELETE'
        });

        closeDeleteModal();
        showToast('Website deleted successfully!', 'success');
        loadSitesList();
    } catch (err) {
        showToast(err.message, 'danger');
    } finally {
        submitBtn.disabled = false;
    }
}

// ======================== HELPERS ========================
function formatBytes(bytes, decimals = 1) {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

function formatDate(isoStr) {
    if (!isoStr) return '';
    try {
        const d = new Date(isoStr);
        return d.toLocaleDateString('en-US', {
            day: '2-digit',
            month: 'short',
            year: 'numeric',
            hour: '2-digit',
            minute: '2-digit'
        });
    } catch {
        return isoStr;
    }
}

function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
        showToast('Copied to clipboard', 'info');
    });
}

function escapeHtml(str) {
    if (!str) return '';
    return str.replace(/[&<>'"]/g, 
        tag => ({
            '&': '&amp;',
            '<': '&lt;',
            '>': '&gt;',
            "'": '&#39;',
            '"': '&quot;'
        }[tag] || tag)
    );
}

function showToast(message, type = 'info') {
    const container = document.getElementById('toast-container');
    const toast = document.createElement('div');
    toast.className = 'toast';

    let iconSvg = '';
    if (type === 'success') {
        iconSvg = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>`;
    } else if (type === 'danger') {
        iconSvg = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--danger)" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>`;
    } else {
        iconSvg = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>`;
    }

    toast.innerHTML = `${iconSvg}<span>${escapeHtml(message)}</span>`;
    container.appendChild(toast);

    setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transform = 'translateY(10px)';
        toast.style.transition = 'all 0.3s ease';
        setTimeout(() => toast.remove(), 300);
    }, 4000);
}
