#!/bin/bash

# ==============================================================================
# Gemini Audit Transfer Script v3
# Usage: ./audit_transfer.sh [A|B] (Defaults to B)
# ==============================================================================

# ------------------------------------------------------------------------------
# 0. Setup & Arguments
# ------------------------------------------------------------------------------
LINE_ID=${1:-B}  # Default to B if not specified
LINE_ID=$(echo "$LINE_ID" | tr '[:lower:]' '[:upper:]') # Convert to uppercase

if [[ "$LINE_ID" != "A" && "$LINE_ID" != "B" ]]; then
    echo "Error: Invalid Line ID '$LINE_ID'. Use 'A' or 'B'."
    exit 1
fi

# Email Configuration
EMAIL_CONFIG=".email_config"
if [ -f "$EMAIL_CONFIG" ]; then
    source "$EMAIL_CONFIG"
fi

send_email_alert() {
    local subject="$1"
    local body="$2"
    
    if [[ -z "$SMTP_USER" || -z "$SMTP_PASS" || -z "$RECIPIENT_EMAIL" ]]; then
        echo -e "${YELLOW}Email config missing. Skipping alert.${NC}"
        return
    fi

    export ALERT_SUBJECT="$subject"
    export ALERT_BODY="$body"
    export SMTP_USER SMTP_PASS RECIPIENT_EMAIL

    python3 -c "
import smtplib, ssl, os
from email.message import EmailMessage

msg = EmailMessage()
msg.set_content(os.environ['ALERT_BODY'].replace('\\\\n', '\\n'))
msg['Subject'] = os.environ['ALERT_SUBJECT']
msg['From'] = os.environ['SMTP_USER']
msg['To'] = os.environ['RECIPIENT_EMAIL']

try:
    context = ssl.create_default_context()
    with smtplib.SMTP_SSL('smtp.gmail.com', 465, context=context) as server:
        server.login(os.environ['SMTP_USER'], os.environ['SMTP_PASS'])
        server.send_message(msg)
    print('Email alert sent successfully.')
except Exception as e:
    print(f'Failed to send email: {e}')
"
}

CONFIG_FILE="Transfer.ps1"
SEARCH_DIR="./Line $LINE_ID"
FOLDER_PREFIX="Archive_Beam_${LINE_ID}_"
TINY_THRESHOLD=1000 

# ANSI Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' 

echo -e "${CYAN}=== Audit Report for LINE $LINE_ID: $(date) ===${NC}"

# ------------------------------------------------------------------------------
# 1. Configuration Parsing
# ------------------------------------------------------------------------------
if [ -f "$CONFIG_FILE" ]; then
    RAW_START=$(sed -n 's/.*$startDate.*"\(.*\)".*/\1/p' "$CONFIG_FILE")
    RAW_END=$(sed -n 's/.*$endDate.*"\(.*\)".*/\1/p' "$CONFIG_FILE")
    if [ -n "$RAW_START" ] && [ -n "$RAW_END" ]; then
        START_DATE=$(date -d "$RAW_START" +%Y-%m-%d)
        END_DATE=$(date -d "$RAW_END" +%Y-%m-%d)
    fi
fi

# ------------------------------------------------------------------------------
# 2. Global Stats
# ------------------------------------------------------------------------------
if [ ! -d "$SEARCH_DIR" ]; then
    echo -e "${YELLOW}Warning: Directory $SEARCH_DIR does not exist.${NC}"
    exit 0
fi

TOTAL_SIZE=$(du -sh "$SEARCH_DIR" | cut -f1)
TOTAL_FILES=$(find "$SEARCH_DIR" -type f | wc -l)
echo -e "Archive Status:  ${GREEN}$TOTAL_SIZE${NC} across ${GREEN}$TOTAL_FILES${NC} files."

# ------------------------------------------------------------------------------
# 3. Live Activity (What is being copied NOW)
# ------------------------------------------------------------------------------
echo -e "\n${CYAN}=== Active Transfer Detection ===${NC}"

RECENT_FILES_RAW=$(find "$SEARCH_DIR" -type f -mmin -5 -exec ls -lh --time-style=long-iso {} + 2>/dev/null)
RECENT_COUNT=$(echo "$RECENT_FILES_RAW" | grep -c "^")

if [ -n "$RECENT_FILES_RAW" ]; then
    RECENT_FILES=$(echo "$RECENT_FILES_RAW" | head -n 3 | awk '{
        size=$5; date=$6; time=$7;
        if (NF < 7) next;
        for(i=1; i<=7; i++) $i="";
        sub(/^[ \t]+/, "", $0);
        print "  - " $0 " (" size ") at " date " " time
    }')

    if [ "$RECENT_COUNT" -gt 3 ]; then
        RECENT_FILES=$(echo -e "$RECENT_FILES\n  ... and $((RECENT_COUNT - 3)) more files.")
    fi
    
    # Final check to ensure it is not just empty/newlines
    if [[ ! "$RECENT_FILES" =~ [a-zA-Z0-9] ]]; then RECENT_FILES=""; fi
else
    RECENT_FILES=""
fi

# Calculate Speed by monitoring total directory growth (Differential Analysis)
get_total_bytes() {
    du -s --block-size=1 "$SEARCH_DIR" | cut -f1
}

SIZE_T1=$(get_total_bytes)
sleep 10
SIZE_T2=$(get_total_bytes)

SPEED_BPS=0
if [ -n "$SIZE_T1" ] && [ -n "$SIZE_T2" ]; then
    # Calculate delta bytes
    DELTA_BYTES=$(( SIZE_T2 - SIZE_T1 ))
    
    # Handle potential negative delta (deletions)
    if [ "$DELTA_BYTES" -lt 0 ]; then DELTA_BYTES=0; fi

    # Bytes per second
    SPEED_BPS=$(( DELTA_BYTES / 10 ))
fi

# Convert to MB/s for display
SPEED_MBS=$(awk -v bps="$SPEED_BPS" 'BEGIN { printf "%.1f", bps/1024/1024 }')

# ------------------------------------------------------------------------------
# 3a. State Tracking & Timestamps
# ------------------------------------------------------------------------------
STATE_FILE=".transfer_state_$LINE_ID"
SINCE_FILE=".transfer_since_$LINE_ID"
CURRENT_STATE="IDLE"
[ "$SPEED_BPS" -gt 0 ] && CURRENT_STATE="ACTIVE"

if [ -f "$STATE_FILE" ]; then
    PREV_STATE=$(cat "$STATE_FILE")
else
    PREV_STATE="IDLE"
fi

# Update Timestamp if state changed or file missing
if [ "$CURRENT_STATE" != "$PREV_STATE" ] || [ ! -f "$SINCE_FILE" ]; then
    date "+%a %b %d %H:%M:%S %Y" > "$SINCE_FILE"
fi
SINCE_TS=$(cat "$SINCE_FILE")

if [ "$SPEED_BPS" -gt 0 ]; then
    echo -e "Status:                 ${GREEN}ACTIVE TRANSFER DETECTED${NC} (since $SINCE_TS)"
    echo -e "Current Transfer Speed: ${GREEN}$SPEED_MBS MB/s${NC} (Specific to Line $LINE_ID)"
else
    echo -e "Status:                 ${YELLOW}IDLE${NC} (since $SINCE_TS)"
    
    # Redundancy Check: dstat
    if command -v dstat &> /dev/null; then
        echo -e "\n${CYAN}=== Redundancy Check (System-Wide Activity) ===${NC}"
        echo "Sampling system I/O (Disk & Network) for 5 seconds..."
        
        # Capture 5 samples (skip headers)
        DSTAT_OUT=$(dstat -dn --nocolor 1 5 | tail -n +3)
        
        # Parse output to detect activity > 1MB/s
        # Columns: read(1) writ(2) | recv(3) send(4)
        # We care about Disk Write (2) and Net Recv (3) mostly.
        
        read BUSY_TYPE AVG_SPEED <<< $(echo "$DSTAT_OUT" | awk '
        function to_bytes(str) {
            val = str + 0;
            if (str ~ /B/) mul = 1;
            else if (str ~ /k/) mul = 1024;
            else if (str ~ /M/) mul = 1048576;
            else if (str ~ /G/) mul = 1073741824;
            else mul = 1; # Default or plain number
            return val * mul;
        }
        {
            disk_wr += to_bytes($2);
            net_rx  += to_bytes($3);
            count++;
        }
        END {
            if (count == 0) exit;
            avg_disk = disk_wr / count;
            avg_net  = net_rx / count;
            
            threshold = 1048576; # 1 MB/s
            
            if (avg_disk > threshold) {
                print "DISK " sprintf("%.1f", avg_disk/1048576) "MB/s";
            } else if (avg_net > threshold) {
                print "NET " sprintf("%.1f", avg_net/1048576) "MB/s";
            } else {
                print "IDLE 0";
            }
        }')

        # Cross-Line Check: Is the other line writing data?
        if [ "$LINE_ID" == "A" ]; then OTHER_LINE="B"; else OTHER_LINE="A"; fi
        OTHER_DIR="./Line $OTHER_LINE"
        IS_OTHER_ACTIVE=$(find "$OTHER_DIR" -type f -mmin -1 -print -quit 2>/dev/null)

        if [ "$BUSY_TYPE" == "DISK" ]; then
            if [ -n "$IS_OTHER_ACTIVE" ]; then
                echo -e "${GREEN}INFO: High Disk Activity ($AVG_SPEED) detected.${NC}"
                echo -e "Activity attributed to concurrent transfer on ${CYAN}Line $OTHER_LINE${NC}."
            else
                echo -e "${YELLOW}WARNING: High System-Wide Disk Activity Detected ($AVG_SPEED).${NC}"
                echo "The transfer might be active but buffering, or another process is writing to disk."
            fi
        elif [ "$BUSY_TYPE" == "NET" ]; then
            if [ -n "$IS_OTHER_ACTIVE" ]; then
                echo -e "${GREEN}INFO: High Network Activity ($AVG_SPEED) detected.${NC}"
                echo -e "Activity attributed to concurrent transfer on ${CYAN}Line $OTHER_LINE${NC}."
            else
                echo -e "${YELLOW}WARNING: High Network Activity Detected ($AVG_SPEED).${NC}"
                echo "Data is being received, but not yet written to the target folder."
            fi
        else
            echo -e "${GREEN}CONFIRMED IDLE:${NC} No significant system-wide Disk or Network activity detected."
        fi
    fi
fi

export SPEED_BPS

if [ -n "$RECENT_FILES" ]; then
    echo -e "${GREEN}Active/Recent File Writes (last 5m):${NC}"
    echo "$RECENT_FILES"
fi

# ------------------------------------------------------------------------------
# 3b. Alert Logic (State Tracking)
# ------------------------------------------------------------------------------
# Note: STATE_FILE, CURRENT_STATE, PREV_STATE are calculated in Section 3a.

if [ "$CURRENT_STATE" != "$PREV_STATE" ]; then
    echo -e "\n${CYAN}--- State Change Detected ($PREV_STATE -> $CURRENT_STATE) ---${NC}"
    
    if [ "$CURRENT_STATE" == "ACTIVE" ]; then
        SUBJECT="[Beam Alert] Transfer RESUMED on Line $LINE_ID"
        BODY="The transfer on Line $LINE_ID has resumed.\n\nCurrent Speed: $SPEED_MBS MB/s\nTime: $(date)"
        send_email_alert "$SUBJECT" "$BODY"
        
    elif [ "$CURRENT_STATE" == "IDLE" ]; then
        # Double-check redundancy before alerting?
        # For now, trust the main logic.
        SUBJECT="[Beam Alert] Transfer STOPPED on Line $LINE_ID"
        BODY="The transfer on Line $LINE_ID has stopped (or paused).\n\nStatus: IDLE\nTime: $(date)\n\nPlease check the dashboard."
        send_email_alert "$SUBJECT" "$BODY"
    fi
    
    echo "$CURRENT_STATE" > "$STATE_FILE"
else
    # Update timestamp of state file even if no change, to show it's fresh
    echo "$CURRENT_STATE" > "$STATE_FILE"
fi

# Remove old /proc/net/dev block
# if [ -f /proc/net/dev ]; then ... fi  <-- DELETED via replacement

# ------------------------------------------------------------------------------
# 4. File Integrity & Heuristics
# ------------------------------------------------------------------------------
echo -e "\n${CYAN}=== File Integrity & Heuristics ===${NC}"

TMP_LIST=$(mktemp)

# Python script to check zip validity quickly
cat << 'EOF' > .fast_zip_check.py
import sys
import os
import zipfile

for line in sys.stdin:
    line = line.strip()
    if not line: continue
    
    parts = line.split('\t')
    if len(parts) < 3: continue
    
    # parts: [filename, size, full_path]
    fpath = parts[2]
    is_valid = 0
    
    try:
        # Fast check: Valid Zip container? (Magic number + EOCD)
        if zipfile.is_zipfile(fpath):
            is_valid = 1
    except:
        pass
        
    print(f"{parts[0]}\t{parts[1]}\t{is_valid}")
EOF

# Find files and pipe to python for checking
# Original format fed to python: filename \t size \t full_path
find "$SEARCH_DIR" -type f -name "*.zip" -printf "%f\t%s\t%p\n" | python3 .fast_zip_check.py > "$TMP_LIST"
rm .fast_zip_check.py

awk -F'\t' -v threshold="$TINY_THRESHOLD" -v red="$RED" -v yellow="$YELLOW" -v nc="$NC" '
function human(x) {
    if (x<1024) return x " B";
    if (x<1048576) return sprintf("%.1f K", x/1024);
    if (x<1073741824) return sprintf("%.1f M", x/1048576);
    return sprintf("%.1f G", x/1073741824);
}
{
    name = $1; size = $2; valid = $3;
    count[name]++;
    
    if (valid == 0) {
        bad_count[name]++;
    }

    if (size < threshold) {
        placeholders[name]++;
    } else {
        # Only add to stats if it is NOT a placeholder (regardless of validity for now)
        # Or should we exclude "Bad" files from stats? 
        # Usually better to include them in size stats but flag them.
        valid_count[name]++;
        valid_sizes[name, valid_count[name]] = size;
        valid_sum[name] += size;
    }
}
END {
    if (NR == 0) { print "No zip files found."; exit; }
    printf "% -25s | % -5s | % -5s | % -5s | % -9s | % -9s | % -9s | % -9s\n", "Device Type", "Total", "Empty", "Bad", "Min", "Max", "Median", "StdDev"
    print "----------------------------------------------------------------------------------------------------------------"
    n = asorti(count, sorted_names)
    for (i = 1; i <= n; i++) {
        name = sorted_names[i]
        v_cnt = valid_count[name];
        if (v_cnt > 0) {
            delete temp_sizes;
            for(k=1; k<=v_cnt; k++) temp_sizes[k] = valid_sizes[name, k];
            min_val = temp_sizes[1]; max_val = temp_sizes[1]; sum_val = 0;
            for(k=1; k<=v_cnt; k++) {
                if(temp_sizes[k] < min_val) min_val = temp_sizes[k];
                if(temp_sizes[k] > max_val) max_val = temp_sizes[k];
                sum_val += temp_sizes[k];
            }
            mean = sum_val / v_cnt;
            asort(temp_sizes);
            if (v_cnt % 2 == 1) median = temp_sizes[int(v_cnt/2) + 1];
            else median = (temp_sizes[v_cnt/2] + temp_sizes[v_cnt/2 + 1]) / 2;
            sq_diff_sum = 0;
            for(k=1; k<=v_cnt; k++) sq_diff_sum += (temp_sizes[k] - mean) ^ 2;
            std_dev = sqrt(sq_diff_sum / v_cnt);
            
            p_color = (placeholders[name] > 0) ? yellow : nc;
            b_color = (bad_count[name] > 0) ? red : nc;
            
            printf "% -25s | % -5d | %s% -5d%s | %s% -5d%s | % -9s | % -9s | % -9s | % -9s\n", 
                name, count[name], p_color, placeholders[name], nc,
                b_color, bad_count[name], nc,
                human(min_val), human(max_val), human(median), human(std_dev)

            # Aggregate for Totals
            grand_total += count[name]
            grand_empty += placeholders[name]
            grand_bad   += bad_count[name]
            
            # Initialize grand min/max with first valid entry
            if (valid_groups == 0) {
                grand_min = min_val
                grand_max = max_val
            } else {
                if (min_val < grand_min) grand_min = min_val
                if (max_val > grand_max) grand_max = max_val
            }
            
            valid_groups++
            all_medians[valid_groups] = median
            all_stddevs[valid_groups] = std_dev
        } else {
             b_color = (bad_count[name] > 0) ? red : nc;
             printf "% -25s | % -5d | %s% -5d%s | %s% -5d%s | % -9s | % -9s | % -9s | % -9s\n", 
                name, count[name], yellow, placeholders[name], nc, b_color, bad_count[name], nc, "-", "-", "-", "-"
             
             # Still count totals even if no valid sizes (just empty files)
             grand_total += count[name]
             grand_empty += placeholders[name]
             grand_bad   += bad_count[name]
        }
    }
    
    # Print Summary Row
    if (valid_groups > 0) {
        print "----------------------------------------------------------------------------------------------------------------"
        
        # Calculate Median of Medians
        asort(all_medians)
        if (valid_groups % 2 == 1) grand_median = all_medians[int(valid_groups/2) + 1]
        else grand_median = (all_medians[valid_groups/2] + all_medians[valid_groups/2 + 1]) / 2

        # Calculate Median of StdDevs
        asort(all_stddevs)
        if (valid_groups % 2 == 1) grand_stddev = all_stddevs[int(valid_groups/2) + 1]
        else grand_stddev = (all_stddevs[valid_groups/2] + all_stddevs[valid_groups/2 + 1]) / 2

        p_color_total = (grand_empty > 0) ? yellow : nc
        b_color_total = (grand_bad > 0) ? red : nc
        
        printf "% -25s | % -5d | %s% -5d%s | %s% -5d%s | % -9s | % -9s | % -9s | % -9s\n",
            "TOTALS / SUMMARY", grand_total, p_color_total, grand_empty, nc,
            b_color_total, grand_bad, nc,
            human(grand_min), human(grand_max), human(grand_median), human(grand_stddev)
    }
}
' "$TMP_LIST"
rm "$TMP_LIST"

# ------------------------------------------------------------------------------
# 5. Gap Analysis (Weekdays Only)
# ------------------------------------------------------------------------------
echo -e "\n${CYAN}=== Gap Analysis (Weekdays Only) ===${NC}"

EXISTING_DATES=$(ls -d "$SEARCH_DIR"/${FOLDER_PREFIX}* 2>/dev/null | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' | sort | uniq)

if [ -n "$EXISTING_DATES" ]; then
    RANGE_START=$(echo "$EXISTING_DATES" | head -n 1)
    RANGE_END=$(echo "$EXISTING_DATES" | tail -n 1)
    CURRENT_CHECK="$RANGE_START"
    WEEKDAY_GAPS=0; WEEKEND_GAPS=0

    while [[ "$CURRENT_CHECK" < "$RANGE_END" ]]; do
        if [ ! -d "$SEARCH_DIR"/${FOLDER_PREFIX}"$CURRENT_CHECK" ]; then
            DOW=$(date -d "$CURRENT_CHECK" +%u)
            if [ "$DOW" -le 5 ]; then
                echo -e "${RED}CRITICAL: Missing Weekday - $CURRENT_CHECK${NC}"
                ((WEEKDAY_GAPS++))
            else
                ((WEEKEND_GAPS++))
            fi
        fi
        CURRENT_CHECK=$(date -I -d "$CURRENT_CHECK + 1 day")
    done
    echo -e "Range Analyzed: $RANGE_START to $RANGE_END"
    if [ "$WEEKDAY_GAPS" -eq 0 ]; then echo -e "${GREEN}No weekday gaps found.${NC} ($WEEKEND_GAPS weekends skipped)";
    else echo -e "${RED}Found $WEEKDAY_GAPS unexpected weekday gaps.${NC}"; fi
fi

# ------------------------------------------------------------------------------
# 6. Estimates
# ------------------------------------------------------------------------------
echo -e "\n${CYAN}=== Transfer Estimates ===${NC}"
CURRENT_COPY_DATE=$(echo "$RECENT_FILES" | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' | head -n 1)
[ -z "$CURRENT_COPY_DATE" ] && CURRENT_COPY_DATE="$RANGE_END"

if [ -n "$CURRENT_COPY_DATE" ] && [ -n "$END_DATE" ]; then
    TOTAL_BYTES=$(du -s --block-size=1 "$SEARCH_DIR" | cut -f1)
    FOLDER_COUNT=$(echo "$EXISTING_DATES" | wc -l)
    [ "$FOLDER_COUNT" -gt 0 ] && AVG_BYTES_PER_DAY=$(( TOTAL_BYTES / FOLDER_COUNT )) || AVG_BYTES_PER_DAY=0
    AVG_GB=$(( AVG_BYTES_PER_DAY / 1024 / 1024 / 1024 ))
    
    WEEKDAYS_LEFT=0; ITER_DATE=$(date -I -d "$CURRENT_COPY_DATE + 1 day")
    while [[ "$ITER_DATE" < "$END_DATE" || "$ITER_DATE" == "$END_DATE" ]]; do
        [ $(date -d "$ITER_DATE" +%u) -le 5 ] && ((WEEKDAYS_LEFT++))
        ITER_DATE=$(date -I -d "$ITER_DATE + 1 day")
    done

    if [ "$WEEKDAYS_LEFT" -gt 0 ]; then
        TOTAL_REMAINING_BYTES=$(( WEEKDAYS_LEFT * AVG_BYTES_PER_DAY ))
        REMAINING_TB=$(awk -v b="$TOTAL_REMAINING_BYTES" 'BEGIN { printf "%.1f", b/1024/1024/1024/1024 }')
        AVAIL_KB=$(df -k "$SEARCH_DIR" | awk 'NR==2 {print $4}')
        REQ_KB=$(( TOTAL_REMAINING_BYTES / 1024 ))
        AVAIL_TB=$(awk -v k="$AVAIL_KB" 'BEGIN { printf "%.1f", k/1024/1024/1024 }')
        
        echo -e "Current Progress: Copying ${GREEN}$CURRENT_COPY_DATE${NC}"
        echo -e "Daily Average:    $AVG_GB GB/day"
        echo -e "Days Remaining:   $WEEKDAYS_LEFT weekdays"
        echo -e "Est. Data Left:   $REMAINING_TB TB (Free: $AVAIL_TB TB)"
        
        if [ "$AVAIL_KB" -gt "$REQ_KB" ]; then echo -e "Disk Status:      ${GREEN}OK${NC}";
        else echo -e "Disk Status:      ${RED}CRITICAL - Insufficient Space!${NC}"; fi

        # SPEED_BPS is already calculated in Section 3
        if [ -z "$SPEED_BPS" ]; then SPEED_BPS=0; fi

        if [ "$SPEED_BPS" -gt 0 ]; then
            SECONDS_LEFT=$(( TOTAL_REMAINING_BYTES / SPEED_BPS ))
            HOURS_LEFT=$(( SECONDS_LEFT / 3600 ))
            DAYS_ETA=$(( HOURS_LEFT / 24 ))
            echo -e "Estimated Time:   ${YELLOW}$DAYS_ETA days${NC} ($HOURS_LEFT hours) at current speed."
        fi
    else echo -e "Transfer appears complete."; fi
fi

# ------------------------------------------------------------------------------
# 7. Directory Size Anomalies
# ------------------------------------------------------------------------------
echo -e "\n${CYAN}=== Directory Size Anomalies ===${NC}"
EXCLUDE_DIR="${FOLDER_PREFIX}$CURRENT_COPY_DATE"
TMP_DIR_SIZES=$(mktemp)
for d in "$SEARCH_DIR"/${FOLDER_PREFIX}*; do
    [ -d "$d" ] || continue
    [ "$(basename "$d")" == "$EXCLUDE_DIR" ] && continue
    echo "$(du -s --block-size=1 "$d" | cut -f1) $(basename "$d")" >> "$TMP_DIR_SIZES"
done
awk -v red="$RED" -v yellow="$YELLOW" -v nc="$NC" '
function human(x) {
    if (x<1024) return x " B"; if (x<1048576) return sprintf("%.1f K", x/1024);
    if (x<1073741824) return sprintf("%.1f M", x/1048576); return sprintf("%.1f G", x/1073741824);
}
{ sizes[NR] = $1; names[NR] = $2; count++ }
END {
    if (count == 0) { print "No completed directories."; exit; }
    asort(sizes, sorted_sizes);
    if (count % 2 == 1) median = sorted_sizes[int(count/2) + 1];
    else median = (sorted_sizes[count/2] + sorted_sizes[count/2 + 1]) / 2;
    print "Median Daily Size: " human(median)
    print "-------------------------------------------------------------------------------"
    for (i = 1; i <= count; i++) {
        if (sizes[i] < median * 0.8) { printf "% -35s | %s% -10s%s | (Too Small)\n", names[i], red, human(sizes[i]), nc; found=1; }
        else if (sizes[i] > median * 1.2) { printf "% -35s | %s% -10s%s | (Too Large)\n", names[i], yellow, human(sizes[i]), nc; found=1; }
    }
    if (!found) print "No significant size anomalies found.";
}
' "$TMP_DIR_SIZES"
rm "$TMP_DIR_SIZES"

echo -e "\n${CYAN}=== Audit Complete ===${NC}"
