
import requests
import json

base_url = "http://localhost:8080"
login_url = f"{base_url}/auth/login"
goal_url = f"{base_url}/goals"

# Login
login_data = {"username": "coach", "password": "CoachPassword123!"}
r = requests.post(login_url, json=login_data)
token = r.json()["token"]

# Create Goal
goal_data = {
  "member_id":      "30000000-0000-0000-0000-000000000001",
  "goal_type":      "fat_loss",
  "title":          "Reduce body fat to 18%",
  "description":    "Steady fat loss over 90 days",
  "start_date":     "2024-01-01",
  "target_date":    "2024-04-01",
  "baseline_value": 22.5,
  "target_value":   18.0
}
headers = {"Authorization": f"Bearer {token}"}
r = requests.post(goal_url, json=goal_data, headers=headers)
print(f"Status: {r.status_code}")
print(f"Body: {r.text}")
