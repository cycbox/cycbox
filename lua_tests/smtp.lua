-- SMTP Mailer Example for CycBox Pro
-- Demonstrates sending emails via SMTP using the smtp_send and smtp_send_async functions

-- Example 1: Send a simple text email (blocking with error return)
function example_simple_email()
    local ok, err = smtp_send({
        server = "smtp.gmail.com",
        port = 587,
        tls = "starttls",
        username = "your-email@gmail.com",
        password = "your-app-password",  -- Use Gmail App Password, not account password
        from = "your-email@gmail.com",
        to = "recipient@example.com",
        subject = "Test Email from CycBox",
        text = "This is a plain text email sent from CycBox Pro Lua script."
    })

    if ok then
        log("info", "Email sent successfully")
    else
        log("error", "Failed to send email: " .. (err or "unknown error"))
    end
end

-- Example 2: Send HTML email (blocking)
function example_html_email()
    local ok, err = smtp_send({
        server = "smtp.gmail.com",
        port = 587,
        tls = "starttls",
        username = "your-email@gmail.com",
        password = "your-app-password",
        from = "your-email@gmail.com",
        to = "recipient@example.com",
        subject = "HTML Email from CycBox",
        html = [[
            <html>
            <body>
                <h1>Hello from CycBox Pro!</h1>
                <p>This is an <strong>HTML</strong> email.</p>
                <ul>
                    <li>Feature 1</li>
                    <li>Feature 2</li>
                </ul>
            </body>
            </html>
        ]]
    })

    if ok then
        log("info", "HTML email sent successfully")
    else
        log("error", "Failed to send HTML email: " .. (err or "unknown error"))
    end
end

-- Example 3: Send email asynchronously (fire-and-forget)
function example_async_email()
    local ok = smtp_send_async({
        server = "smtp.gmail.com",
        port = 587,
        tls = "starttls",
        username = "your-email@gmail.com",
        password = "your-app-password",
        from = "your-email@gmail.com",
        to = "recipient@example.com",
        subject = "Async Email from CycBox",
        text = "This email was sent asynchronously. Errors are logged, not returned."
    })

    if ok then
        log("info", "Email queued for sending")
    else
        log("error", "Failed to queue email (validation error)")
    end
end

-- Example 4: Send email using Direct TLS (port 465)
function example_tls_email()
    local ok, err = smtp_send({
        server = "smtp.gmail.com",
        port = 465,
        tls = "tls",  -- Direct TLS connection
        username = "your-email@gmail.com",
        password = "your-app-password",
        from = "your-email@gmail.com",
        to = "recipient@example.com",
        subject = "TLS Email from CycBox",
        text = "This email uses direct TLS connection (port 465)."
    })

    if ok then
        log("info", "TLS email sent successfully")
    else
        log("error", "Failed to send TLS email: " .. (err or "unknown error"))
    end
end

-- Example 5: Send alert email when threshold is exceeded
function on_receive()
    -- Get numeric value from message
    local values = message:get_values()
    if #values > 0 then
        local value = values[1].value

        -- Check if value exceeds threshold
        if value > 100 then
            -- Send alert email asynchronously
            smtp_send_async({
                server = "smtp.gmail.com",
                port = 587,
                tls = "starttls",
                username = "your-email@gmail.com",
                password = "your-app-password",
                from = "alerts@cycbox.com",
                to = "admin@example.com",
                subject = "Alert: Threshold Exceeded",
                text = string.format("Alert: Value %.2f exceeded threshold of 100 at %s",
                    value, os.date("%Y-%m-%d %H:%M:%S"))
            })

            log("warn", string.format("Alert email sent for value: %.2f", value))
        end
    end

    return false  -- Don't modify message
end

-- TLS Modes:
-- "starttls" - Upgrade to TLS after initial connection (default, port 587)
-- "tls"      - Direct TLS connection (port 465)
-- "none"     - Unencrypted connection (port 25, not recommended for public servers)

--[[
{
  "version": "1.10.0",
  "name": "SMTP send email",
  "description": "Serial debugging assistant",
  "configs": [
    {
      "app": {
        "app_transport": "udp",
        "app_codec": "timeout_codec",
        "app_transformer": "disable",
        "app_encoding": "UTF-8"
      },
      "udp": {
        "udp_bind_address": "0.0.0.0",
        "udp_bind_port": 5000,
        "udp_enable_broadcast": false,
        "udp_enable_multicast": false,
        "udp_multicast_groups": "239.255.0.1\nff02::1\n",
        "udp_multicast_ttl": 1,
        "udp_multicast_hop_limit": 1,
        "udp_multicast_loopback": true
      },
      "timeout_codec": {
        "with_receive_timeout": 100
      }
    }
  ]
}
]]
