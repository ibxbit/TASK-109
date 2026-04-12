#!/bin/bash
TOKEN=$(curl -s -X POST http://app:8080/auth/login -H 'Content-Type: application/json' -d '{"username":"coach","password":"Coach1234!"}' | jq -r .token)
echo "Token: $TOKEN"
curl -i -X POST http://app:8080/goals \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "member_id": "30000000-0000-0000-0000-000000000001",
    "goal_type": "fat_loss",
    "title": "Reduce body fat to 18%",
    "description": "Steady fat loss over 90 days",
    "start_date": "2024-01-01",
    "target_date": "2024-04-01",
    "baseline_value": 22.5,
    "target_value": 18.0
  }'
