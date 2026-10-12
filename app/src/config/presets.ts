// Configuration > User-Agent presets. Each fills the HTTP request
// user-agent and the robots.txt user-agent token. Browser presets use a
// blank robots token, which obeys only the `*` group. The AI crawler
// strings follow each vendor's published format [unverified versions].

export interface UserAgentPreset {
  name: string;
  userAgent: string;
  robots: string;
}

const CHROME = "141.0.0.0";

export const USER_AGENT_PRESETS: UserAgentPreset[] = [
  {
    name: "Googlebot (Smartphone)",
    userAgent: `Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${CHROME} Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)`,
    robots: "Googlebot",
  },
  {
    name: "Googlebot (Desktop)",
    userAgent: `Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Googlebot/2.1; +http://www.google.com/bot.html) Chrome/${CHROME} Safari/537.36`,
    robots: "Googlebot",
  },
  { name: "Googlebot Image", userAgent: "Googlebot-Image/1.0", robots: "Googlebot-Image" },
  { name: "Googlebot News", userAgent: "Googlebot-News", robots: "Googlebot-News" },
  {
    name: "Google-InspectionTool (Smartphone)",
    userAgent: `Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${CHROME} Mobile Safari/537.36 (compatible; Google-InspectionTool/1.0;)`,
    robots: "Google-InspectionTool",
  },
  {
    name: "Bingbot",
    userAgent: `Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm) Chrome/${CHROME} Safari/537.36`,
    robots: "bingbot",
  },
  {
    name: "Chrome",
    userAgent: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${CHROME} Safari/537.36`,
    robots: "",
  },
  {
    name: "Chrome for Android",
    userAgent: `Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${CHROME} Mobile Safari/537.36`,
    robots: "",
  },
  { name: "Firefox", userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:143.0) Gecko/20100101 Firefox/143.0", robots: "" },
  {
    name: "Safari",
    userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Safari/605.1.15",
    robots: "",
  },
  {
    name: "Edge",
    userAgent: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${CHROME} Safari/537.36 Edg/${CHROME}`,
    robots: "",
  },
  { name: "GPTBot", userAgent: "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.2; +https://openai.com/gptbot)", robots: "GPTBot" },
  { name: "ClaudeBot", userAgent: "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; ClaudeBot/1.0; +claudebot@anthropic.com)", robots: "ClaudeBot" },
  {
    name: "PerplexityBot",
    userAgent: "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; PerplexityBot/1.0; +https://perplexity.ai/perplexitybot)",
    robots: "PerplexityBot",
  },
];

/** Name suggestions for Configuration > HTTP Header. */
export const HEADER_NAMES = [
  "Accept",
  "Accept-Encoding",
  "Accept-Language",
  "Authorization",
  "Cache-Control",
  "Connection",
  "Cookie",
  "Content-Type",
  "DNT",
  "Forwarded",
  "From",
  "If-Modified-Since",
  "Origin",
  "Pragma",
  "Referer",
  "Sec-Fetch-Mode",
  "Upgrade-Insecure-Requests",
  "X-Forwarded-For",
  "X-Requested-With",
];

/** RFC 7230 token: what a header name may contain. */
export const validHeaderName = (name: string) => /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(name);
