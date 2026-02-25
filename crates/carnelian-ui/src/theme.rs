//! Glassy UI theme for Carnelian OS desktop application.
//!
//! Defines color palette, typography, and CSS utility classes
//! for a translucent dark theme with blur effects.

/// UI theme selection used as Dioxus context.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub dark: bool,
}

impl Theme {
    pub fn new_dark() -> Self {
        Self { dark: true }
    }

    /// Return the CSS class for this theme.
    pub fn to_class(&self) -> &'static str {
        if self.dark {
            "theme-dark"
        } else {
            "theme-light"
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new_dark()
    }
}

// Color palette — used by CSS and available for future component styling.
#[allow(dead_code)]
pub const BG_PRIMARY: &str = "rgba(10, 10, 20, 0.85)";
#[allow(dead_code)]
pub const BG_SECONDARY: &str = "rgba(20, 20, 35, 0.75)";
#[allow(dead_code)]
pub const BG_PANEL: &str = "rgba(30, 30, 50, 0.65)";
#[allow(dead_code)]
pub const ACCENT_BLUE: &str = "#4A90E2";
#[allow(dead_code)]
pub const ACCENT_PURPLE: &str = "#9B59B6";
#[allow(dead_code)]
pub const TEXT_PRIMARY: &str = "#E0E0E0";
#[allow(dead_code)]
pub const TEXT_SECONDARY: &str = "#A0A0A0";
#[allow(dead_code)]
pub const BORDER_SUBTLE: &str = "rgba(255, 255, 255, 0.1)";

// Status colors
#[allow(dead_code)]
pub const STATUS_CONNECTED: &str = "#2ECC71";
#[allow(dead_code)]
pub const STATUS_CONNECTING: &str = "#F39C12";
#[allow(dead_code)]
pub const STATUS_DISCONNECTED: &str = "#E74C3C";

// Profile badge colors
#[allow(dead_code)]
pub const PROFILE_THUMMIM: &str = "#4A90E2";
#[allow(dead_code)]
pub const PROFILE_URIM: &str = "#9B59B6";
#[allow(dead_code)]
pub const PROFILE_CUSTOM: &str = "#7F8C8D";

/// Global CSS styles for the application.
///
/// Includes base styles, glass panel effects, typography,
/// layout utilities, and component-specific styles.
pub const GLOBAL_CSS: &str = r#"
/* ============================================================
   BASE STYLES
   ============================================================ */

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
    font-size: 14px;
    line-height: 1.5;
    color: #E0E0E0;
    background: linear-gradient(135deg, #0a0a14 0%, #1a1028 50%, #0a0a14 100%);
    min-height: 100vh;
    overflow: hidden;
    -webkit-font-smoothing: antialiased;
}

/* ============================================================
   TYPOGRAPHY
   ============================================================ */

h1 { font-size: 24px; font-weight: 600; color: #E0E0E0; }
h2 { font-size: 20px; font-weight: 600; color: #E0E0E0; }
h3 { font-size: 16px; font-weight: 600; color: #E0E0E0; }

/* ============================================================
   GLASS PANEL
   ============================================================ */

.glass-panel {
    background: rgba(30, 30, 50, 0.65);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
}

/* ============================================================
   LAYOUT
   ============================================================ */

.app-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
}

.main-content {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
}

.flex-row {
    display: flex;
    flex-direction: row;
    align-items: center;
}

.flex-col {
    display: flex;
    flex-direction: column;
}

/* ============================================================
   TOP BAR
   ============================================================ */

.top-bar {
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 20px;
    background: rgba(10, 10, 20, 0.85);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    flex-shrink: 0;
}

.top-bar-left {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 18px;
    font-weight: 700;
    color: #E0E0E0;
}

.top-bar-center {
    display: flex;
    align-items: center;
    gap: 8px;
}

.top-bar-right {
    display: flex;
    align-items: center;
    gap: 12px;
}

/* ============================================================
   STATUS INDICATOR
   ============================================================ */

.status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    display: inline-block;
}

.status-dot.connected { background: #2ECC71; box-shadow: 0 0 6px #2ECC71; }
.status-dot.connecting { background: #F39C12; animation: pulse 1.5s infinite; }
.status-dot.disconnected { background: #E74C3C; }

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
}

.status-label {
    font-size: 12px;
    color: #A0A0A0;
}

/* ============================================================
   PROFILE BADGE
   ============================================================ */

.badge {
    display: inline-flex;
    align-items: center;
    padding: 4px 12px;
    border-radius: 16px;
    font-size: 12px;
    font-weight: 600;
    color: #E0E0E0;
}

.badge-thummim { background: rgba(74, 144, 226, 0.3); border: 1px solid rgba(74, 144, 226, 0.5); }
.badge-urim { background: rgba(155, 89, 182, 0.3); border: 1px solid rgba(155, 89, 182, 0.5); }
.badge-custom { background: rgba(127, 140, 141, 0.3); border: 1px solid rgba(127, 140, 141, 0.5); }

/* ============================================================
   SETTINGS BUTTON
   ============================================================ */

.btn-icon {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #A0A0A0;
    cursor: pointer;
    padding: 6px 10px;
    font-size: 16px;
    transition: all 0.2s ease;
}

.btn-icon:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #E0E0E0;
    border-color: rgba(255, 255, 255, 0.2);
}

/* ============================================================
   TAB NAVIGATION
   ============================================================ */

.tab-nav {
    height: 50px;
    display: flex;
    align-items: center;
    gap: 0;
    background: rgba(20, 20, 35, 0.75);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
}

.tab-link {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #A0A0A0;
    text-decoration: none;
    font-size: 14px;
    font-weight: 500;
    border-bottom: 3px solid transparent;
    transition: all 0.2s ease;
    cursor: pointer;
}

.tab-link:hover {
    color: #E0E0E0;
    background: rgba(255, 255, 255, 0.04);
}

.tab-link.active {
    color: #E0E0E0;
    border-bottom: 3px solid;
    border-image: linear-gradient(90deg, #4A90E2, #9B59B6) 1;
}

/* ============================================================
   PAGE CONTENT
   ============================================================ */

.page-panel {
    padding: 24px;
}

.page-panel h1 {
    margin-bottom: 12px;
}

.page-panel p {
    color: #A0A0A0;
}

/* ============================================================
   PADDING UTILITIES
   ============================================================ */

.p-6 { padding: 24px; }
.p-4 { padding: 16px; }
.p-2 { padding: 8px; }

/* ============================================================
   TEXT UTILITIES
   ============================================================ */

.text-primary { color: #E0E0E0; }
.text-secondary { color: #A0A0A0; }

/* ============================================================
   SCROLLBAR
   ============================================================ */

::-webkit-scrollbar {
    width: 6px;
}

::-webkit-scrollbar-track {
    background: rgba(10, 10, 20, 0.5);
}

::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.15);
    border-radius: 3px;
}

::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.25);
}

/* ============================================================
   WINDOW CONTROLS (tray menu fallback)
   ============================================================ */

.window-controls {
    display: flex;
    align-items: center;
    gap: 4px;
}

.btn-window-control {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    padding: 0;
}

.btn-quit:hover {
    background: rgba(231, 76, 60, 0.3) !important;
    border-color: rgba(231, 76, 60, 0.5) !important;
    color: #E74C3C !important;
}

/* ============================================================
   TRAY STATUS BADGE
   ============================================================ */

.tray-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 600;
}

.tray-icon { font-size: 10px; }
.tray-label { color: #A0A0A0; }

.tray-running { background: rgba(46, 204, 113, 0.15); }
.tray-running .tray-label { color: #2ECC71; }

.tray-connecting { background: rgba(243, 156, 18, 0.15); }
.tray-connecting .tray-label { color: #F39C12; }

.tray-stopped { background: rgba(231, 76, 60, 0.15); }
.tray-stopped .tray-label { color: #E74C3C; }

.tray-error { background: rgba(231, 76, 60, 0.15); }
.tray-error .tray-label { color: #E74C3C; }

/* ============================================================
   SYSTEM STATUS (top bar)
   ============================================================ */

.system-status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
}

.system-status-badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 10px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 600;
}

.system-status-badge.healthy {
    background: rgba(46, 204, 113, 0.15);
    color: #2ECC71;
}

.system-status-badge.unhealthy {
    background: rgba(231, 76, 60, 0.15);
    color: #E74C3C;
}

.system-version {
    color: #7F8C8D;
    font-size: 11px;
}

.system-uptime {
    color: #7F8C8D;
    font-size: 11px;
}

/* ============================================================
   DATA TABLE
   ============================================================ */

.data-table {
    width: 100%;
    border-collapse: collapse;
    background: rgba(30, 30, 50, 0.65);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    overflow: hidden;
}

.data-table thead {
    background: rgba(20, 20, 35, 0.8);
}

.data-table th {
    padding: 10px 14px;
    text-align: left;
    font-size: 12px;
    font-weight: 600;
    color: #A0A0A0;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    cursor: pointer;
    user-select: none;
    white-space: nowrap;
}

.data-table th:hover {
    color: #E0E0E0;
}

.data-table th .sort-indicator {
    margin-left: 4px;
    font-size: 10px;
    opacity: 0.6;
}

.data-table tbody tr {
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    transition: background 0.15s ease;
}

.data-table tbody tr:hover {
    background: rgba(74, 144, 226, 0.08);
}

.data-table td {
    padding: 10px 14px;
    font-size: 13px;
    color: #E0E0E0;
    vertical-align: middle;
}

.data-table .cell-truncate {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.data-table .cell-mono {
    font-family: 'Cascadia Code', 'Fira Code', monospace;
    font-size: 12px;
    color: #A0A0A0;
}

.row-highlight {
    animation: row-flash 1.5s ease-out;
}

@keyframes row-flash {
    0% { background: rgba(74, 144, 226, 0.25); }
    100% { background: transparent; }
}

/* ============================================================
   FILTER BAR
   ============================================================ */

.filter-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 0;
    flex-wrap: wrap;
}

.filter-bar-actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
}

.filter-input {
    background: rgba(20, 20, 35, 0.75);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #E0E0E0;
    padding: 7px 12px;
    font-size: 13px;
    outline: none;
    transition: border-color 0.2s ease;
    min-width: 180px;
}

.filter-input::placeholder {
    color: #7F8C8D;
}

.filter-input:focus {
    border-color: rgba(74, 144, 226, 0.5);
}

.filter-select {
    background: rgba(20, 20, 35, 0.75);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #E0E0E0;
    padding: 7px 12px;
    font-size: 13px;
    outline: none;
    cursor: pointer;
    appearance: none;
    -webkit-appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M0 0l5 6 5-6z' fill='%23A0A0A0'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 10px center;
    padding-right: 28px;
}

.filter-select:focus {
    border-color: rgba(74, 144, 226, 0.5);
}

.filter-select option {
    background: #1a1028;
    color: #E0E0E0;
}

.filter-checkbox {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #A0A0A0;
    cursor: pointer;
}

.filter-checkbox input[type='checkbox'] {
    accent-color: #4A90E2;
}

/* ============================================================
   BUTTONS
   ============================================================ */

.btn-primary {
    background: linear-gradient(135deg, #4A90E2, #357ABD);
    border: 1px solid rgba(74, 144, 226, 0.5);
    border-radius: 6px;
    color: #fff;
    padding: 7px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
}

.btn-primary:hover {
    background: linear-gradient(135deg, #5BA0F2, #4A90E2);
    box-shadow: 0 2px 8px rgba(74, 144, 226, 0.3);
}

.btn-secondary {
    background: rgba(127, 140, 141, 0.2);
    border: 1px solid rgba(127, 140, 141, 0.3);
    border-radius: 6px;
    color: #A0A0A0;
    padding: 7px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
}

.btn-secondary:hover {
    background: rgba(127, 140, 141, 0.3);
    color: #E0E0E0;
}

.btn-danger {
    background: rgba(231, 76, 60, 0.2);
    border: 1px solid rgba(231, 76, 60, 0.3);
    border-radius: 6px;
    color: #E74C3C;
    padding: 7px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
}

.btn-danger:hover {
    background: rgba(231, 76, 60, 0.3);
    box-shadow: 0 2px 8px rgba(231, 76, 60, 0.2);
}

.btn-success {
    background: rgba(46, 204, 113, 0.2);
    border: 1px solid rgba(46, 204, 113, 0.3);
    border-radius: 6px;
    color: #2ECC71;
    padding: 7px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
}

.btn-success:hover {
    background: rgba(46, 204, 113, 0.3);
    box-shadow: 0 2px 8px rgba(46, 204, 113, 0.2);
}

.btn-sm {
    padding: 4px 10px;
    font-size: 12px;
}

/* ============================================================
   STATUS BADGES
   ============================================================ */

.badge-status {
    display: inline-flex;
    align-items: center;
    padding: 3px 10px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
}

.badge-pending {
    background: rgba(243, 156, 18, 0.15);
    color: #F39C12;
}

.badge-running {
    background: rgba(74, 144, 226, 0.15);
    color: #4A90E2;
    animation: pulse 1.5s infinite;
}

.badge-completed {
    background: rgba(46, 204, 113, 0.15);
    color: #2ECC71;
}

.badge-failed {
    background: rgba(231, 76, 60, 0.15);
    color: #E74C3C;
}

.badge-cancelled {
    background: rgba(127, 140, 141, 0.15);
    color: #7F8C8D;
}

.badge-enabled {
    background: rgba(46, 204, 113, 0.15);
    color: #2ECC71;
}

.badge-disabled {
    background: rgba(127, 140, 141, 0.15);
    color: #7F8C8D;
}

/* ============================================================
   MODAL
   ============================================================ */

.modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.modal-content {
    background: rgba(25, 25, 45, 0.95);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    min-width: 480px;
    max-width: 720px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
}

.modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.modal-header h2 {
    font-size: 16px;
    font-weight: 600;
}

.modal-body {
    padding: 20px;
    overflow-y: auto;
    flex: 1;
}

.modal-body pre {
    background: rgba(10, 10, 20, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    padding: 14px;
    font-family: 'Cascadia Code', 'Fira Code', monospace;
    font-size: 12px;
    color: #E0E0E0;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-word;
}

.modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}

/* ============================================================
   METRIC CARDS (Dashboard)
   ============================================================ */

.metrics-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 16px;
    margin-bottom: 24px;
}

.metric-card {
    background: rgba(30, 30, 50, 0.65);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    transition: border-color 0.2s ease;
}

.metric-card:hover {
    border-color: rgba(255, 255, 255, 0.18);
}

.metric-value {
    font-size: 32px;
    font-weight: 700;
    color: #E0E0E0;
    line-height: 1;
}

.metric-label {
    font-size: 12px;
    font-weight: 500;
    color: #A0A0A0;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.metric-card.metric-active .metric-value { color: #4A90E2; }
.metric-card.metric-pending .metric-value { color: #F39C12; }
.metric-card.metric-completed .metric-value { color: #2ECC71; }
.metric-card.metric-failed .metric-value { color: #E74C3C; }

/* ============================================================
   GAUGE (Dashboard resource usage)
   ============================================================ */

.gauges-row {
    display: flex;
    gap: 24px;
    margin-bottom: 24px;
}

.gauge-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
}

.gauge-container svg {
    transform: rotate(-90deg);
}

.gauge-background {
    fill: none;
    stroke: rgba(255, 255, 255, 0.06);
}

.gauge-fill {
    fill: none;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.6s ease;
}

.gauge-label {
    font-size: 13px;
    color: #A0A0A0;
    font-weight: 500;
}

.gauge-value-text {
    font-size: 18px;
    font-weight: 700;
    fill: #E0E0E0;
}

/* ============================================================
   SECTION HEADERS
   ============================================================ */

.section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
}

.section-header h2 {
    font-size: 16px;
    font-weight: 600;
}

/* ============================================================
   HEALTH INDICATORS (Dashboard)
   ============================================================ */

.health-row {
    display: flex;
    gap: 16px;
    margin-bottom: 24px;
}

.health-indicator {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(30, 30, 50, 0.65);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 12px 18px;
    font-size: 13px;
    color: #E0E0E0;
}

.health-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
}

.health-dot.healthy { background: #2ECC71; box-shadow: 0 0 6px #2ECC71; }
.health-dot.unhealthy { background: #E74C3C; }
.health-dot.unknown { background: #7F8C8D; }

/* ============================================================
   EVENT LIST (Events panel)
   ============================================================ */

.event-list {
    flex: 1;
    overflow-y: auto;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    background: rgba(20, 20, 35, 0.5);
}

.event-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    font-size: 13px;
    cursor: pointer;
    transition: background 0.15s ease;
}

.event-row:hover {
    background: rgba(74, 144, 226, 0.06);
}

.event-row.level-error {
    background: rgba(231, 76, 60, 0.06);
}

.event-row.level-warn {
    background: rgba(243, 156, 18, 0.04);
}

.event-timestamp {
    font-family: 'Cascadia Code', 'Fira Code', monospace;
    font-size: 11px;
    color: #7F8C8D;
    white-space: nowrap;
    min-width: 90px;
}

.event-level-badge {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    min-width: 44px;
    justify-content: center;
}

.event-level-badge.error { background: rgba(231, 76, 60, 0.2); color: #E74C3C; }
.event-level-badge.warn { background: rgba(243, 156, 18, 0.2); color: #F39C12; }
.event-level-badge.info { background: rgba(74, 144, 226, 0.2); color: #4A90E2; }
.event-level-badge.debug { background: rgba(127, 140, 141, 0.2); color: #95A5A6; }
.event-level-badge.trace { background: rgba(127, 140, 141, 0.1); color: #7F8C8D; }

.event-type {
    font-weight: 500;
    color: #E0E0E0;
    white-space: nowrap;
    min-width: 140px;
}

.event-actor {
    color: #9B59B6;
    font-size: 12px;
    white-space: nowrap;
}

.event-message {
    color: #A0A0A0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.event-controls {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 0;
}

.auto-scroll-btn {
    background: rgba(74, 144, 226, 0.15);
    border: 1px solid rgba(74, 144, 226, 0.3);
    border-radius: 6px;
    color: #4A90E2;
    padding: 5px 12px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.2s ease;
}

.auto-scroll-btn:hover {
    background: rgba(74, 144, 226, 0.25);
}

.auto-scroll-btn.active {
    background: rgba(74, 144, 226, 0.3);
    border-color: rgba(74, 144, 226, 0.5);
}

/* ============================================================
   PAGINATION
   ============================================================ */

.pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 12px 0;
}

.pagination-btn {
    background: rgba(30, 30, 50, 0.65);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #A0A0A0;
    padding: 6px 14px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s ease;
}

.pagination-btn:hover:not(:disabled) {
    background: rgba(74, 144, 226, 0.15);
    color: #E0E0E0;
    border-color: rgba(74, 144, 226, 0.3);
}

.pagination-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.pagination-info {
    font-size: 13px;
    color: #A0A0A0;
}

.page-size-select {
    background: rgba(20, 20, 35, 0.75);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #E0E0E0;
    padding: 4px 8px;
    font-size: 12px;
    outline: none;
}

/* ============================================================
   FORM FIELDS (Create Task modal)
   ============================================================ */

.form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
}

.form-label {
    font-size: 12px;
    font-weight: 600;
    color: #A0A0A0;
    text-transform: uppercase;
    letter-spacing: 0.3px;
}

.form-input {
    background: rgba(10, 10, 20, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #E0E0E0;
    padding: 9px 12px;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s ease;
}

.form-input:focus {
    border-color: rgba(74, 144, 226, 0.5);
}

.form-textarea {
    background: rgba(10, 10, 20, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #E0E0E0;
    padding: 9px 12px;
    font-size: 14px;
    outline: none;
    resize: vertical;
    min-height: 80px;
    font-family: inherit;
    transition: border-color 0.2s ease;
}

.form-textarea:focus {
    border-color: rgba(74, 144, 226, 0.5);
}

/* ============================================================
   EMPTY / LOADING / ERROR STATES
   ============================================================ */

.state-message {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px 24px;
    color: #7F8C8D;
    font-size: 14px;
    text-align: center;
    gap: 8px;
}

.state-message .state-icon {
    font-size: 32px;
    opacity: 0.5;
}

.spinner {
    width: 24px;
    height: 24px;
    border: 3px solid rgba(255, 255, 255, 0.1);
    border-top-color: #4A90E2;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
}

@keyframes spin {
    to { transform: rotate(360deg); }
}

/* ============================================================
   PANEL LAYOUT HELPERS
   ============================================================ */

.panel-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    gap: 0;
}

.panel-scroll {
    flex: 1;
    overflow-y: auto;
}

/* ============================================================
   WORKFLOW BUILDER
   ============================================================ */

.wf-builder-container {
    display: flex;
    flex: 1;
    gap: 0;
    overflow: hidden;
    min-height: 0;
}

.wf-sidebar {
    width: 220px;
    min-width: 220px;
    padding: 12px;
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    overflow-y: auto;
    background: rgba(15, 15, 30, 0.4);
}

.wf-sidebar h3 {
    font-size: 13px;
    font-weight: 600;
    color: #E0E0E0;
    margin: 0 0 8px 0;
}

.wf-skill-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.wf-skill-card {
    padding: 8px 10px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
}

.wf-skill-card:hover {
    background: rgba(74, 144, 226, 0.1);
    border-color: rgba(74, 144, 226, 0.3);
}

.wf-skill-card-name {
    font-size: 12px;
    font-weight: 600;
    color: #E0E0E0;
    margin-bottom: 4px;
}

.wf-runtime-node { background: rgba(104, 211, 145, 0.15); color: #68D391; }
.wf-runtime-python { background: rgba(246, 173, 85, 0.15); color: #F6AD55; }
.wf-runtime-shell { background: rgba(159, 122, 234, 0.15); color: #9F7AEA; }

.wf-canvas-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
}

.wf-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(15, 15, 30, 0.3);
}

.wf-canvas {
    flex: 1;
    overflow: auto;
    background: rgba(10, 10, 25, 0.6);
    border-radius: 4px;
    margin: 8px;
}

.wf-canvas svg {
    display: block;
}

.wf-config-panel {
    width: 260px;
    min-width: 260px;
    padding: 12px;
    border-left: 1px solid rgba(255, 255, 255, 0.06);
    overflow-y: auto;
    background: rgba(15, 15, 30, 0.4);
}

.wf-config-content h3 {
    font-size: 13px;
    font-weight: 600;
    color: #E0E0E0;
    margin: 0 0 4px 0;
}

.wf-config-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 20px;
    text-align: center;
}

.wf-dep-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.wf-validation-errors {
    padding: 8px;
    margin-bottom: 8px;
}

.wf-validation-error {
    color: #FC8181;
    font-size: 12px;
    padding: 4px 0;
}

/* Execution modal results */

.wf-exec-results {
    margin-top: 12px;
}

.wf-exec-summary {
    display: flex;
    align-items: center;
    margin-bottom: 12px;
}

.wf-exec-timeline {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.wf-exec-step {
    padding: 8px 12px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.02);
}

.wf-exec-step-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
}

.wf-exec-step-name {
    font-weight: 600;
    color: #E0E0E0;
}

.wf-exec-step-error {
    color: #FC8181;
    font-size: 12px;
    margin-top: 4px;
}

.wf-step-success { border-left: 3px solid #68D391; }
.wf-step-failed  { border-left: 3px solid #FC8181; }
.wf-step-skipped { border-left: 3px solid #A0AEC0; }
.wf-step-pending { border-left: 3px solid #F6AD55; }
.wf-step-running { border-left: 3px solid #4A90E2; animation: pulse 1.5s infinite; }

.wf-field-error {
    border-color: #FC8181 !important;
    box-shadow: 0 0 0 1px rgba(252, 129, 129, 0.3);
}
.wf-field-error-msg {
    color: #FC8181;
    font-size: 11px;
    margin-top: 4px;
}

.badge-enabled  { background: rgba(104, 211, 145, 0.15); color: #68D391; }
.badge-disabled { background: rgba(160, 174, 192, 0.15); color: #A0AEC0; }

.btn-danger {
    background: rgba(252, 129, 129, 0.15);
    color: #FC8181;
    border: 1px solid rgba(252, 129, 129, 0.3);
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: background 0.15s;
}

.btn-danger:hover {
    background: rgba(252, 129, 129, 0.25);
}

.btn-danger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

.filter-checkbox {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #B0B0B0;
    cursor: pointer;
}

.filter-checkbox input[type="checkbox"] {
    accent-color: #4A90E2;
}

.form-textarea {
    width: 100%;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #E0E0E0;
    padding: 8px 10px;
    font-size: 13px;
    resize: vertical;
    min-height: 60px;
}

.form-textarea:focus {
    outline: none;
    border-color: rgba(74, 144, 226, 0.5);
}

.cell-truncate {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.filter-bar-actions {
    display: flex;
    gap: 6px;
    margin-left: auto;
}

.modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
}

.modal-header h2 {
    margin: 0;
    font-size: 18px;
    color: #E0E0E0;
}

.modal-body {
    margin-bottom: 16px;
}

.modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
}

.btn-icon {
    background: none;
    border: none;
    color: #7F8C8D;
    font-size: 18px;
    cursor: pointer;
    padding: 4px;
}

.btn-icon:hover {
    color: #E0E0E0;
}

/* ============================================================
   XP LEVEL BADGE (top bar + widget)
   ============================================================ */

.xp-level-badge {
    display: inline-flex;
    align-items: center;
    padding: 3px 10px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 700;
    color: #fff;
    background: linear-gradient(135deg, #D4A017, #F5C842);
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
    white-space: nowrap;
}

/* ============================================================
   XP PROGRESS BAR
   ============================================================ */

.xp-progress-bar-container {
    width: 80px;
    height: 6px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 3px;
    overflow: hidden;
}

.xp-progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #D4A017, #F5C842);
    border-radius: 3px;
    transition: width 0.4s ease;
}

.xp-progress-label {
    font-size: 11px;
    color: #A0A0A0;
    white-space: nowrap;
}

/* ============================================================
   XP WIDGET CARD
   ============================================================ */

.xp-widget {
    background: rgba(30, 30, 50, 0.65);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 20px;
    margin-bottom: 24px;
    transition: border-color 0.2s ease;
}

.xp-widget:hover {
    border-color: rgba(212, 160, 23, 0.3);
}

/* ============================================================
   TOAST NOTIFICATIONS
   ============================================================ */

.toast-container {
    position: fixed;
    top: 70px;
    right: 20px;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
}

.toast {
    background: rgba(30, 30, 50, 0.92);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    padding: 10px 16px;
    font-size: 13px;
    color: #E0E0E0;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    animation: toast-slide-in 0.3s ease-out;
    pointer-events: auto;
}

.toast-level-up {
    border-color: rgba(212, 160, 23, 0.5);
    font-size: 15px;
    font-weight: 700;
    animation: toast-slide-in 0.3s ease-out, toast-pulse 1.5s ease-in-out 2;
}

@keyframes toast-slide-in {
    0% { transform: translateX(100%); opacity: 0; }
    100% { transform: translateX(0); opacity: 1; }
}

@keyframes toast-pulse {
    0%, 100% { box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4); }
    50% { box-shadow: 0 4px 24px rgba(212, 160, 23, 0.4); }
}

/* ============================================================
   XP PIE CHART
   ============================================================ */

.xp-pie-chart {
    width: 80px;
    height: 80px;
    flex-shrink: 0;
}

.xp-pie-chart svg {
    display: block;
}

/* ============================================================
   VOICE SETTINGS PANEL
   ============================================================ */

.voice-settings-panel {
    background: rgba(30, 30, 50, 0.65);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 24px;
    max-width: 560px;
}
"#;
