//! Glassy UI theme for Carnelian OS desktop application.
//!
//! Defines color palette, typography, and CSS utility classes
//! for a translucent dark theme with blur effects.

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
pub const GLOBAL_CSS: &str = r"
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
";
