#!/bin/bash
# AWS S3 bucket list wrapper - formats input/output for S3 operations
set -e

# Read JSON from stdin
input=$(cat)

# Extract bucket name if provided, otherwise list all buckets
bucket=$(echo "$input" | jq -r '.arguments.bucket // empty')

if [ -n "$bucket" ]; then
    # List objects in specific bucket
    aws s3 ls "s3://$bucket/" 2>/dev/null || echo "Error: Unable to access bucket $bucket"
else
    # List all S3 buckets
    aws s3 ls 2>/dev/null || echo "No buckets found or AWS not configured"
fi
