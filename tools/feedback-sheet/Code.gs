/**
 * Held in Common feedback → Google Sheet (decision record 0030).
 *
 * A Google Apps Script web app. A Held in Common server (node or commons)
 * POSTs one JSON message per submission (format "hic-feedback/1", built in
 * commons/src/feedback.rs). This script checks it and adds a row:
 *
 *   - every message with words goes to the "Feedback" tab;
 *   - anyone who ticked "Add me to the mailing list" goes to the
 *     "Mailing list" tab (once per email address).
 *
 * It answers {"ok": true} when it kept the message, or
 * {"ok": false, "error": "..."} when it didn't. The server only shows
 * "Sent" for {"ok": true}.
 *
 * Why it checks everything again: the web app's address is in every copy of
 * the app, so anyone could post to it. The checks below refuse anything
 * that isn't the shape our form sends, and a rough rate limit stops a flood.
 *
 * Set-up steps are in README.md next to this file.
 */

/** The format name the server puts in every message. */
var FORMAT = 'hic-feedback/1';

/** Limits, matching commons/src/feedback.rs. */
var MESSAGE_MAX = 5000;
var EMAIL_MAX = 254;

/** The kinds the form offers. */
var KINDS = ['bug', 'idea', 'other'];

/** At most this many messages in any 10-minute window, from everyone. */
var RATE_LIMIT = 60;

/** Tab names, and their header rows (written when a tab is first made). */
var FEEDBACK_TAB = 'Feedback';
var FEEDBACK_HEADERS = ['Received', 'Kind', 'Message', 'Email', 'May reply', 'Mailing list', 'App version', 'From', 'Status', 'Notes'];
var LIST_TAB = 'Mailing list';
var LIST_HEADERS = ['Joined', 'Email', 'App version'];

/**
 * Handle one POST from a Held in Common server. Writes at most one row to
 * each tab, under a lock so two messages at once can't collide.
 */
function doPost(e) {
  var message;
  try {
    message = JSON.parse(e.postData.contents);
  } catch (err) {
    return answer(false, 'not JSON');
  }

  var problem = check(message);
  if (problem) {
    return answer(false, problem);
  }
  if (!underRateLimit()) {
    return answer(false, 'too many messages; try again later');
  }

  var lock = LockService.getScriptLock();
  lock.waitLock(10000);
  try {
    var book = SpreadsheetApp.getActiveSpreadsheet();
    var now = new Date();
    if (message.message) {
      tab(book, FEEDBACK_TAB, FEEDBACK_HEADERS).appendRow([
        now,
        message.kind,
        safe(message.message),
        safe(message.email),
        message.may_reply ? 'yes' : 'no',
        message.mailing_list ? 'yes' : 'no',
        safe(message.version),
        message.role,
        'New',
        '',
      ]);
    }
    if (message.mailing_list && message.email) {
      addToMailingList(tab(book, LIST_TAB, LIST_HEADERS), message, now);
    }
  } finally {
    lock.releaseLock();
  }
  return answer(true);
}

/**
 * A GET (someone opening the address in a browser) changes nothing; it
 * only says what this is.
 */
function doGet() {
  return ContentService.createTextOutput('Held in Common feedback: POST only.');
}

/**
 * What's wrong with `m`, or '' when it's the shape our form sends. Only
 * the fields the server sends, of the right types and lengths.
 */
function check(m) {
  var allowed = ['format', 'kind', 'message', 'email', 'may_reply', 'mailing_list', 'version', 'role'];
  if (!m || typeof m !== 'object') return 'not an object';
  for (var key in m) {
    if (allowed.indexOf(key) === -1) return 'unexpected field ' + key;
  }
  if (m.format !== FORMAT) return 'unknown format';
  if (typeof m.message !== 'string' || typeof m.email !== 'string' || typeof m.version !== 'string') return 'bad field';
  if (typeof m.may_reply !== 'boolean' || typeof m.mailing_list !== 'boolean') return 'bad field';
  if (m.role !== 'node' && m.role !== 'commons') return 'bad role';
  if (m.message.length > MESSAGE_MAX || m.email.length > EMAIL_MAX || m.version.length > 40) return 'too long';
  if (!m.message && !m.mailing_list) return 'empty';
  if (m.message && KINDS.indexOf(m.kind) === -1) return 'unknown kind';
  if ((m.may_reply || m.mailing_list) && !m.email) return 'needs an email';
  if (m.email && !/^[^@\s]+@[^@\s.][^@\s]*\.[^@\s]+$/.test(m.email)) return 'bad email';
  return '';
}

/**
 * Count this message against the 10-minute limit. Returns false when the
 * limit is reached. Writes the count to the script cache (never to the sheet).
 */
function underRateLimit() {
  var cache = CacheService.getScriptCache();
  var key = 'count-' + Math.floor(Date.now() / 600000);
  var count = Number(cache.get(key) || 0);
  if (count >= RATE_LIMIT) return false;
  cache.put(key, String(count + 1), 900);
  return true;
}

/**
 * Add `m.email` to the mailing list tab unless it's already there
 * (compared without case). Writes at most one row.
 */
function addToMailingList(sheet, m, now) {
  var email = m.email.toLowerCase();
  var rows = sheet.getLastRow();
  if (rows > 1) {
    var existing = sheet.getRange(2, 2, rows - 1, 1).getValues();
    for (var i = 0; i < existing.length; i++) {
      if (String(existing[i][0]).toLowerCase() === email) return;
    }
  }
  sheet.appendRow([now, safe(m.email), safe(m.version)]);
}

/**
 * The tab called `name`, made with a bold, frozen header row if it
 * doesn't exist yet.
 */
function tab(book, name, headers) {
  var sheet = book.getSheetByName(name);
  if (!sheet) {
    sheet = book.insertSheet(name);
    sheet.appendRow(headers);
    sheet.getRange(1, 1, 1, headers.length).setFontWeight('bold');
    sheet.setFrozenRows(1);
  }
  return sheet;
}

/**
 * Text as a sheet cell that can't run as a formula: anything starting with
 * = + - or @ gets a leading apostrophe, which the sheet hides. Stops a
 * message like "=IMPORTXML(...)" from doing anything when the team opens it.
 */
function safe(text) {
  return /^[=+\-@]/.test(text) ? "'" + text : text;
}

/** The JSON answer the server reads. */
function answer(ok, error) {
  var body = ok ? { ok: true } : { ok: false, error: error };
  return ContentService.createTextOutput(JSON.stringify(body)).setMimeType(ContentService.MimeType.JSON);
}
