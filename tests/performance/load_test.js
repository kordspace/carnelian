// k6 Load Testing Script for CARNELIAN
//
// Tests API endpoints under various load conditions
// Run with: k6 run tests/performance/load_test.js

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const skillExecutionDuration = new Trend('skill_execution_duration');
const memoryQueryDuration = new Trend('memory_query_duration');

// Test configuration
export const options = {
    stages: [
        { duration: '30s', target: 10 },   // Warm up: ramp to 10 users
        { duration: '1m', target: 50 },    // Load test: ramp to 50 users
        { duration: '2m', target: 50 },    // Sustained load: stay at 50 users
        { duration: '1m', target: 100 },   // Stress test: ramp to 100 users
        { duration: '1m', target: 100 },   // Peak load: stay at 100 users
        { duration: '30s', target: 0 },    // Cool down: ramp down to 0
    ],
    thresholds: {
        http_req_duration: ['p(95)<500'],           // 95% of requests under 500ms
        http_req_failed: ['rate<0.01'],             // Less than 1% failure rate
        errors: ['rate<0.05'],                      // Less than 5% error rate
        skill_execution_duration: ['p(95)<1000'],   // 95% of skill executions under 1s
        memory_query_duration: ['p(95)<200'],       // 95% of memory queries under 200ms
    },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const API_KEY = __ENV.API_KEY || 'test_api_key';

const headers = {
    'Content-Type': 'application/json',
    'X-Carnelian-Key': API_KEY,
};

export default function() {
    // Test 1: Health check
    const healthResponse = http.get(`${BASE_URL}/health`);
    check(healthResponse, {
        'health check status is 200': (r) => r.status === 200,
    });

    // Test 2: List skills
    const skillsResponse = http.get(`${BASE_URL}/api/skills`, { headers });
    check(skillsResponse, {
        'list skills status is 200': (r) => r.status === 200,
        'list skills response time < 200ms': (r) => r.timings.duration < 200,
    });

    // Test 3: Execute skill
    const skillPayload = JSON.stringify({
        skill_name: 'test-skill',
        input: {
            action: 'execute',
            params: { data: 'test data ' + Date.now() }
        },
        timeout_secs: 30
    });

    const skillStart = Date.now();
    const skillResponse = http.post(`${BASE_URL}/api/skills/execute`, skillPayload, { headers });
    const skillDuration = Date.now() - skillStart;
    
    skillExecutionDuration.add(skillDuration);
    
    const skillSuccess = check(skillResponse, {
        'skill execution status is 200 or 404': (r) => r.status === 200 || r.status === 404,
        'skill execution response time < 1000ms': (r) => r.timings.duration < 1000,
    });
    
    if (!skillSuccess) {
        errorRate.add(1);
    } else {
        errorRate.add(0);
    }

    // Test 4: Create memory
    const memoryPayload = JSON.stringify({
        content: `Load test memory created at ${Date.now()}`,
        metadata: {
            test: true,
            timestamp: Date.now()
        },
        tags: ['load-test', 'performance']
    });

    const createMemoryResponse = http.post(`${BASE_URL}/api/memories`, memoryPayload, { headers });
    check(createMemoryResponse, {
        'create memory status is 201': (r) => r.status === 201,
        'create memory response time < 300ms': (r) => r.timings.duration < 300,
    });

    // Test 5: List memories
    const memoryStart = Date.now();
    const listMemoriesResponse = http.get(`${BASE_URL}/api/memories?limit=10&offset=0`, { headers });
    const memoryDuration = Date.now() - memoryStart;
    
    memoryQueryDuration.add(memoryDuration);
    
    check(listMemoriesResponse, {
        'list memories status is 200': (r) => r.status === 200,
        'list memories response time < 200ms': (r) => r.timings.duration < 200,
    });

    // Test 6: Get XP leaderboard
    const leaderboardResponse = http.get(`${BASE_URL}/api/xp/leaderboard?limit=10`, { headers });
    check(leaderboardResponse, {
        'leaderboard status is 200': (r) => r.status === 200,
        'leaderboard response time < 300ms': (r) => r.timings.duration < 300,
    });

    // Test 7: Award XP
    const xpPayload = JSON.stringify({
        identity_id: '00000000-0000-0000-0000-000000000001',
        amount: Math.floor(Math.random() * 100) + 1,
        source: 'skill_execution',
        description: 'Load test XP award'
    });

    const xpResponse = http.post(`${BASE_URL}/api/xp/award`, xpPayload, { headers });
    check(xpResponse, {
        'award XP status is 200': (r) => r.status === 200,
        'award XP response time < 200ms': (r) => r.timings.duration < 200,
    });

    // Test 8: List workflows
    const workflowsResponse = http.get(`${BASE_URL}/api/workflows?limit=10`, { headers });
    check(workflowsResponse, {
        'list workflows status is 200': (r) => r.status === 200,
    });

    // Random sleep between 0.5 and 2 seconds to simulate real user behavior
    sleep(Math.random() * 1.5 + 0.5);
}

// Setup function (runs once at the beginning)
export function setup() {
    console.log('Starting CARNELIAN load test...');
    console.log(`Base URL: ${BASE_URL}`);
    console.log(`Target stages: 10 -> 50 -> 100 users`);
    
    // Verify server is accessible
    const healthCheck = http.get(`${BASE_URL}/health`);
    if (healthCheck.status !== 200) {
        throw new Error('Server health check failed. Is CARNELIAN running?');
    }
    
    return { startTime: Date.now() };
}

// Teardown function (runs once at the end)
export function teardown(data) {
    const duration = (Date.now() - data.startTime) / 1000;
    console.log(`Load test completed in ${duration.toFixed(2)} seconds`);
}
