#!/usr/bin/env python3
"""
Radicale Log Analyzer
=====================

Parses radicale system logs from journalctl and generates:
1. A CSV file with date, client (name + IP), request type, bytes, processing type
2. Plot of date vs total data transfers per client (with markers for differentiation)
3. Plot of date vs total data transfers per request type (with markers and units)

Usage:
    python analyze_radicale_logs.py [--days N] [--output-dir DIR]
"""

import argparse
import csv
import re
import subprocess
from collections import defaultdict
from datetime import datetime, timedelta
import os
import sys

try:
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    import matplotlib.dates as mdates
    from matplotlib.ticker import FuncFormatter
    MATPLOTLIB_AVAILABLE = True
except ImportError:
    MATPLOTLIB_AVAILABLE = False
    print("Warning: matplotlib not available. Plots will not be generated.")


def format_bytes(bytes_val):
    """Format bytes into appropriate unit (B, KB, MB, GB, TB)."""
    if bytes_val >= 1024**4:
        return f"{bytes_val / 1024**4:.2f} TB"
    elif bytes_val >= 1024**3:
        return f"{bytes_val / 1024**3:.2f} GB"
    elif bytes_val >= 1024**2:
        return f"{bytes_val / 1024**2:.2f} MB"
    elif bytes_val >= 1024:
        return f"{bytes_val / 1024:.2f} KB"
    else:
        return f"{bytes_val} B"


def get_journalctl_logs(days=30):
    since_date = (datetime.now() - timedelta(days=days)).strftime('%Y-%m-%d')
    cmd = ['journalctl', '-u', 'radicale', '--no-pager', '--since', since_date, '--until', 'now']
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        if result.returncode != 0:
            print(f"Error running journalctl: {result.stderr}", file=sys.stderr)
            return None
        return result.stdout
    except subprocess.TimeoutExpired:
        print("Error: journalctl command timed out", file=sys.stderr)
        return None
    except FileNotFoundError:
        print("Error: journalctl not found.", file=sys.stderr)
        return None


def extract_thread_id(line):
    match = re.search(r'\[\d+/Thread-(\d+)', line)
    return match.group(1) if match else None


def parse_date(date_str, current_year):
    try:
        date_obj = datetime.strptime(f"{current_year} {date_str}", '%Y %b %d %H:%M:%S')
        return date_obj.strftime('%Y-%m-%d'), date_obj
    except ValueError:
        pass
    try:
        date_obj = datetime.strptime(date_str, '%b %d %H:%M:%S')
        return date_obj.replace(year=current_year).strftime('%Y-%m-%d'), date_obj.replace(year=current_year)
    except ValueError:
        pass
    return date_str, None


def parse_logs_with_context(log_text):
    entries = []
    lines = log_text.split('\n')
    thread_client_info = {}
    current_year = datetime.now().year
    request_pattern = r'^(\w+ \d+ \d+:\d+:\d+) .*? (\w+) request for .*? received from (\S+) using \'([^\']+)\''
    response_pattern = r'^(\w+ \d+ \d+:\d+:\d+) .*? (\w+) response status for .*? in [0-9.]+ seconds (\w+) (\d+) bytes(?: \(([^)]+)\))?:'
    alt_response_pattern = r'^(\w+ \d+ \d+:\d+:\d+) .*? (\w+) response .*? received from (\S+) using \'([^\']+)\'.*?(plain|gzip|deflate|compress) (\d+) bytes(?: \(([^)]+)\))?:'
    
    for line in lines:
        if not line.strip():
            continue
        thread_id = extract_thread_id(line)
        
        request_match = re.match(request_pattern, line)
        if request_match:
            date_str, req_type, client_ip, client_name = request_match.groups()
            date_only, _ = parse_date(date_str, current_year)
            if thread_id:
                thread_client_info[thread_id] = {'ip': client_ip, 'name': client_name, 'date': date_only, 'request_type': req_type}
            continue
        
        response_match = re.match(response_pattern, line)
        if response_match:
            date_str, req_type, encoding, bytes_str, processing_type = response_match.groups()
            date_only, _ = parse_date(date_str, current_year)
            client_ip, client_name = '', ''
            if thread_id and thread_id in thread_client_info:
                client_ip = thread_client_info[thread_id]['ip']
                client_name = thread_client_info[thread_id]['name']
            processing_type = processing_type.strip() if processing_type else ''
            entries.append({'date': date_only, 'time': date_str, 'client_ip': client_ip, 'client_name': client_name,
                           'request_type': req_type, 'request_category': 'response', 'bytes': int(bytes_str),
                           'encoding': encoding, 'processing_type': processing_type, 'thread_id': thread_id, 'line': line.strip()})
            continue
        
        alt_match = re.match(alt_response_pattern, line)
        if alt_match:
            date_str, req_type, client_ip, client_name, encoding, bytes_str, processing_type = alt_match.groups()
            date_only, _ = parse_date(date_str, current_year)
            processing_type = processing_type.strip() if processing_type else ''
            entries.append({'date': date_only, 'time': date_str, 'client_ip': client_ip, 'client_name': client_name,
                           'request_type': req_type, 'request_category': 'response', 'bytes': int(bytes_str),
                           'encoding': encoding, 'processing_type': processing_type, 'thread_id': thread_id, 'line': line.strip()})
            continue
        
        byte_match = re.search(r'(plain|gzip|deflate|compress) (\d+) bytes', line)
        if byte_match:
            encoding = byte_match.group(1)
            bytes_str = byte_match.group(2)
            date_match = re.match(r'^(\w+ \d+ \d+:\d+:\d+)', line)
            date_str, date_only = '', ''
            if date_match:
                date_str = date_match.group(1)
                date_only, _ = parse_date(date_str, current_year)
            req_type_match = re.search(r'(GET|POST|PUT|DELETE|PROPFIND|REPORT|OPTIONS|HEAD)', line)
            req_type = req_type_match.group(1) if req_type_match else 'UNKNOWN'
            client_ip, client_name = '', ''
            using_match = re.search(r'received from (\S+) using \'([^\']+)\'', line)
            if using_match:
                client_ip, client_name = using_match.groups()
            elif thread_id and thread_id in thread_client_info:
                client_ip = thread_client_info[thread_id]['ip']
                client_name = thread_client_info[thread_id]['name']
            processing_type = ''
            proc_match = re.search(r'\(([^)]+)\)', line)
            if proc_match:
                processing_type = proc_match.group(1).strip()
            entries.append({'date': date_only, 'time': date_str, 'client_ip': client_ip, 'client_name': client_name,
                           'request_type': req_type, 'request_category': 'unknown', 'bytes': int(bytes_str),
                           'encoding': encoding, 'processing_type': processing_type, 'thread_id': thread_id, 'line': line.strip()})
    return entries


def generate_csv(entries, output_path):
    fieldnames = ['date', 'client', 'client_ip', 'client_name', 'request_type', 'request_category', 'bytes', 'encoding', 'processing_type']
    with open(output_path, 'w', newline='') as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
        writer.writeheader()
        for entry in entries:
            client_full = f"{entry['client_name']} ({entry['client_ip']})" if entry['client_ip'] and entry['client_name'] else entry['client_ip'] or entry['client_name'] or 'Unknown'
            writer.writerow({'date': entry['date'], 'client': client_full, 'client_ip': entry['client_ip'],
                           'client_name': entry['client_name'], 'request_type': entry['request_type'],
                           'request_category': entry['request_category'], 'bytes': entry['bytes'],
                           'encoding': entry['encoding'], 'processing_type': entry['processing_type']})
    print(f"CSV file generated: {output_path}")
    return output_path


def aggregate_data_by_client(entries):
    data = defaultdict(lambda: defaultdict(int))
    for entry in entries:
        client_full = f"{entry['client_name']} ({entry['client_ip']})" if entry['client_ip'] and entry['client_name'] else entry['client_ip'] or entry['client_name'] or 'Unknown'
        data[client_full][entry['date']] += entry['bytes']
    return data


def aggregate_data_by_request_type(entries):
    data = defaultdict(lambda: defaultdict(int))
    for entry in entries:
        data[entry['request_type']][entry['date']] += entry['bytes']
    return data


def yaxis_formatter(y, pos):
    """Format Y-axis values in human-readable units."""
    return format_bytes(y)


def plot_data(client_data, request_data, output_dir):
    if not MATPLOTLIB_AVAILABLE:
        print("Skipping plots: matplotlib not available")
        return
    os.makedirs(output_dir, exist_ok=True)
    all_dates = set()
    for dates in client_data.values():
        all_dates.update(dates.keys())
    for dates in request_data.values():
        all_dates.update(dates.keys())
    sorted_dates = sorted(all_dates)
    markers = ['o', 's', '^', 'D', 'v', '<', '>', 'p', '*', 'h', 'H', '+', 'x', 'd', 'P', 'X', '|', '_']
    
    # Plot 1: Data transfers per client
    plt.figure(figsize=(14, 8))
    client_totals = sorted([(c, sum(d.values()), d) for c, d in client_data.items()], key=lambda x: x[1], reverse=True)
    for idx, (client, total, dates) in enumerate(client_totals):
        client_dates, client_bytes = [], []
        for date in sorted_dates:
            if date in dates:
                client_dates.append(datetime.strptime(date, '%Y-%m-%d'))
                client_bytes.append(dates[date])
        if client_dates:
            plt.plot(client_dates, client_bytes, marker=markers[idx % len(markers)], markersize=4, label=f"{client} ({format_bytes(total)})")
    plt.title('Total Data Transfers by Client Over Time')
    plt.xlabel('Date')
    plt.ylabel('Total Data Transferred')
    plt.legend(bbox_to_anchor=(1.05, 1), loc='upper left', fontsize=8)
    plt.grid(True, alpha=0.3)
    plt.gca().xaxis.set_major_formatter(mdates.DateFormatter('%Y-%m-%d'))
    plt.gca().xaxis.set_major_locator(mdates.DayLocator(interval=1))
    plt.gca().yaxis.set_major_formatter(FuncFormatter(yaxis_formatter))
    plt.gcf().autofmt_xdate()
    plt.tight_layout()
    clients_plot_path = os.path.join(output_dir, 'data_by_client.png')
    plt.savefig(clients_plot_path, dpi=300, bbox_inches='tight')
    print(f"Plot saved: {clients_plot_path}")
    plt.close()
    
    # Plot 2: Data transfers per request type
    plt.figure(figsize=(14, 8))
    req_totals = sorted([(r, sum(d.values()), d) for r, d in request_data.items()], key=lambda x: x[1], reverse=True)
    for idx, (req_type, total, dates) in enumerate(req_totals):
        req_dates, req_bytes = [], []
        for date in sorted_dates:
            if date in dates:
                req_dates.append(datetime.strptime(date, '%Y-%m-%d'))
                req_bytes.append(dates[date])
        if req_dates:
            plt.plot(req_dates, req_bytes, marker=markers[idx % len(markers)], markersize=4, label=f"{req_type} ({format_bytes(total)})")
    plt.title('Total Data Transfers by Request Type Over Time')
    plt.xlabel('Date')
    plt.ylabel('Total Data Transferred')
    plt.legend(bbox_to_anchor=(1.05, 1), loc='upper left', fontsize=8)
    plt.grid(True, alpha=0.3)
    plt.gca().xaxis.set_major_formatter(mdates.DateFormatter('%Y-%m-%d'))
    plt.gca().xaxis.set_major_locator(mdates.DayLocator(interval=1))
    plt.gca().yaxis.set_major_formatter(FuncFormatter(yaxis_formatter))
    plt.gcf().autofmt_xdate()
    plt.tight_layout()
    request_plot_path = os.path.join(output_dir, 'data_by_request_type.png')
    plt.savefig(request_plot_path, dpi=300, bbox_inches='tight')
    print(f"Plot saved: {request_plot_path}")
    plt.close()


def print_summary(entries, client_data, request_data):
    print("\n" + "="*80)
    print("SUMMARY")
    print("="*80)
    print(f"\nTotal log entries with byte data: {len(entries):,}")
    total_bytes = sum(e['bytes'] for e in entries)
    print(f"Total bytes transferred: {format_bytes(total_bytes)}")
    print("\nTop 10 Clients by Total Data:")
    for client, total in sorted([(c, sum(d.values())) for c, d in client_data.items()], key=lambda x: x[1], reverse=True)[:10]:
        print(f"  {client}: {format_bytes(total)}")
    print("\nTop Request Types by Total Data:")
    for req_type, total in sorted([(r, sum(d.values())) for r, d in request_data.items()], key=lambda x: x[1], reverse=True):
        print(f"  {req_type}: {format_bytes(total)}")
    print("\nData by Encoding:")
    encodings = defaultdict(int)
    for e in entries:
        encodings[e['encoding']] += e['bytes']
    for enc, total in sorted(encodings.items(), key=lambda x: x[1], reverse=True):
        print(f"  {enc}: {format_bytes(total)}")
    processing = defaultdict(int)
    for e in entries:
        if e['processing_type']:
            processing[e['processing_type']] += e['bytes']
    if processing:
        print("\nData by Processing Type:")
        for proc, total in sorted(processing.items(), key=lambda x: x[1], reverse=True):
            print(f"  {proc}: {format_bytes(total)}")
    print("="*80 + "\n")


def main():
    parser = argparse.ArgumentParser(description='Analyze Radicale logs')
    parser.add_argument('--days', type=int, default=30, help='Number of days (default: 30)')
    parser.add_argument('--output-dir', type=str, default='./radicale_analysis', help='Output directory (default: ./radicale_analysis)')
    parser.add_argument('--csv-only', action='store_true', help='Only generate CSV')
    parser.add_argument('--no-plots', action='store_true', help='Skip plots')
    args = parser.parse_args()
    
    print(f"Analyzing Radicale logs for the last {args.days} days...")
    log_text = get_journalctl_logs(args.days)
    if log_text is None:
        print("Failed to fetch logs.", file=sys.stderr)
        sys.exit(1)
    print(f"Fetched {len(log_text.splitlines()):,} log lines")
    print("Parsing logs...")
    entries = parse_logs_with_context(log_text)
    print(f"Found {len(entries):,} entries with byte information")
    if not entries:
        print("No entries with byte information found.")
        sys.exit(0)
    os.makedirs(args.output_dir, exist_ok=True)
    csv_path = os.path.join(args.output_dir, 'radicale_data_usage.csv')
    generate_csv(entries, csv_path)
    print("Aggregating data...")
    client_data = aggregate_data_by_client(entries)
    request_data = aggregate_data_by_request_type(entries)
    print_summary(entries, client_data, request_data)
    if not args.csv_only and not args.no_plots and MATPLOTLIB_AVAILABLE:
        print("Generating plots...")
        plot_data(client_data, request_data, args.output_dir)
    elif args.csv_only or args.no_plots:
        print("Skipping plot generation.")
    print(f"\nAnalysis complete! CSV: {csv_path}")
    if not args.csv_only and not args.no_plots and MATPLOTLIB_AVAILABLE:
        print(f"Plots: {args.output_dir}/data_by_client.png, {args.output_dir}/data_by_request_type.png")


if __name__ == '__main__':
    main()
