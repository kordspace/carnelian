# CARNELIAN Security Checklist

Comprehensive security checklist for production deployment.

---

## Pre-Deployment Security Audit

### 1. Authentication & Authorization ✓

- [x] API key authentication implemented
- [x] API key validation on all protected endpoints
- [x] Unauthorized requests return 401
- [x] Invalid API keys rejected
- [ ] API key rotation mechanism
- [ ] Multi-factor authentication (future)
- [ ] Role-based access control (future)

### 2. Secrets Management ✓

- [x] Docker secrets support implemented
- [x] Environment variable fallback
- [x] Secrets directory gitignored
- [x] No hardcoded credentials in code
- [ ] Secrets rotation policy documented
- [ ] Encrypted secrets at rest
- [ ] Secrets audit logging

**Action Items:**
```bash
# Create production secrets
echo "your_secure_password" > secrets/postgres_password.txt
echo "your_api_key" > secrets/carnelian_api_key.txt
chmod 600 secrets/*
```

### 3. Network Security ✓

- [x] HTTPS enforcement (HSTS headers)
- [x] CORS configuration (production mode)
- [x] Rate limiting (10 req/s, burst 20)
- [x] Security headers (CSP, X-Frame-Options, etc.)
- [ ] TLS 1.3 enforcement
- [ ] Certificate pinning
- [ ] DDoS protection

**Verify:**
```bash
curl -I https://your-domain.com/health | grep -i "strict-transport-security"
```

### 4. Input Validation ✓

- [x] Content-Type validation
- [x] Request size limits (10MB)
- [x] JSON structure validation
- [x] SQL injection prevention
- [x] XSS prevention
- [x] Input sanitization
- [ ] File upload validation
- [ ] Command injection prevention

### 5. Database Security

- [x] Parameterized queries (SQLx)
- [x] Connection pooling
- [x] Foreign key constraints
- [ ] Database encryption at rest
- [ ] Database backup encryption
- [ ] Audit logging for sensitive operations
- [ ] Row-level security policies

**Action Items:**
```sql
-- Enable PostgreSQL audit logging
ALTER SYSTEM SET log_statement = 'mod';
ALTER SYSTEM SET log_connections = 'on';
```

### 6. Security Headers ✓

- [x] Content-Security-Policy
- [x] Strict-Transport-Security (HSTS)
- [x] X-Frame-Options
- [x] X-Content-Type-Options
- [x] X-XSS-Protection
- [x] Referrer-Policy
- [x] Permissions-Policy
- [ ] Expect-CT header

**Verify:**
```bash
curl -I https://your-domain.com | grep -E "Content-Security-Policy|X-Frame-Options|Strict-Transport-Security"
```

### 7. Error Handling

- [x] Generic error messages (no stack traces)
- [x] Structured logging
- [ ] Error rate monitoring
- [ ] Alerting on error spikes
- [ ] Sanitized error responses

**Check:**
```bash
# Ensure errors don't leak sensitive info
curl https://your-domain.com/api/nonexistent
```

### 8. Dependency Security

- [ ] Regular dependency updates
- [ ] Vulnerability scanning (cargo-audit)
- [ ] License compliance
- [ ] Supply chain security

**Run:**
```bash
cargo install cargo-audit
cargo audit
cargo outdated
```

### 9. Container Security

- [ ] Non-root user in containers
- [ ] Minimal base images
- [ ] Image vulnerability scanning
- [ ] Read-only root filesystem
- [ ] Resource limits enforced
- [ ] Network policies

**Docker Security:**
```dockerfile
# Use non-root user
USER carnelian:carnelian

# Read-only root filesystem
--read-only --tmpfs /tmp
```

### 10. Monitoring & Logging

- [x] Prometheus metrics
- [x] Grafana dashboards
- [x] Structured logging
- [ ] Security event logging
- [ ] Log aggregation (ELK/Loki)
- [ ] Alerting rules
- [ ] Incident response plan

---

## Security Testing

### Static Analysis

```bash
# Run security audit
./scripts/security-audit.sh

# Check for vulnerabilities
cargo audit

# Lint for security issues
cargo clippy -- -W clippy::all
```

### Dynamic Testing

```bash
# Run penetration tests
# Use tools like OWASP ZAP, Burp Suite

# SQL injection testing
sqlmap -u "http://localhost:8080/api/endpoint" --batch

# XSS testing
# Test with payloads like <script>alert(1)</script>
```

### Load Testing

```bash
# Test rate limiting
k6 run tests/performance/load_test.js

# Verify rate limits work
for i in {1..30}; do curl http://localhost:8080/api/skills; done
```

---

## Production Deployment Checklist

### Before Deployment

- [ ] Run security audit: `./scripts/security-audit.sh`
- [ ] Run all tests: `cargo test --all`
- [ ] Run benchmarks: `cargo bench`
- [ ] Update dependencies: `cargo update`
- [ ] Scan for vulnerabilities: `cargo audit`
- [ ] Review recent code changes
- [ ] Update documentation
- [ ] Create backup of current production

### Deployment

- [ ] Use production machine profile
- [ ] Configure Docker secrets
- [ ] Set production environment variables
- [ ] Enable HTTPS/TLS
- [ ] Configure firewall rules
- [ ] Set up monitoring
- [ ] Configure log aggregation
- [ ] Test rollback procedure

### After Deployment

- [ ] Run deployment validation: `./scripts/validate-deployment.sh`
- [ ] Verify all services running
- [ ] Check security headers
- [ ] Test authentication
- [ ] Monitor error rates
- [ ] Verify backups working
- [ ] Test alerting
- [ ] Document deployment

---

## Security Incident Response

### Detection

1. Monitor logs for suspicious activity
2. Set up alerts for:
   - Failed authentication attempts (>10/min)
   - Rate limit violations
   - Unusual error rates
   - Database connection failures
   - Unexpected API usage patterns

### Response

1. **Identify**: Determine scope and impact
2. **Contain**: Isolate affected systems
3. **Eradicate**: Remove threat
4. **Recover**: Restore services
5. **Learn**: Post-incident review

### Emergency Contacts

```
Security Team: security@example.com
On-Call Engineer: +1-XXX-XXX-XXXX
Incident Commander: name@example.com
```

---

## Compliance

### GDPR (if applicable)

- [ ] Data encryption at rest
- [ ] Data encryption in transit
- [ ] Right to erasure implemented
- [ ] Data portability
- [ ] Privacy policy
- [ ] Cookie consent
- [ ] Data breach notification procedure

### SOC 2 (if applicable)

- [ ] Access controls
- [ ] Audit logging
- [ ] Encryption
- [ ] Monitoring
- [ ] Incident response
- [ ] Vendor management

---

## Security Best Practices

### Code Review

- Review all PRs for security issues
- Use automated security scanning
- Follow secure coding guidelines
- Validate all inputs
- Sanitize all outputs

### Access Control

- Principle of least privilege
- Regular access reviews
- Revoke access promptly
- Use strong passwords
- Enable MFA where possible

### Data Protection

- Encrypt sensitive data
- Minimize data collection
- Regular data cleanup
- Secure data backups
- Test restore procedures

### Network Security

- Use VPNs for remote access
- Segment networks
- Monitor network traffic
- Update firewall rules
- Regular security audits

---

## Security Tools

### Recommended Tools

- **cargo-audit**: Dependency vulnerability scanning
- **cargo-deny**: License and security policy enforcement
- **OWASP ZAP**: Web application security scanner
- **Trivy**: Container vulnerability scanner
- **Falco**: Runtime security monitoring
- **Vault**: Secrets management

### Installation

```bash
# Rust security tools
cargo install cargo-audit
cargo install cargo-deny

# Container scanning
docker pull aquasec/trivy
trivy image carnelian:latest
```

---

## Regular Security Tasks

### Daily

- Monitor security alerts
- Review failed authentication attempts
- Check error logs

### Weekly

- Review access logs
- Update dependencies
- Run vulnerability scans
- Review security metrics

### Monthly

- Security audit
- Penetration testing
- Access review
- Backup testing
- Incident response drill

### Quarterly

- Full security assessment
- Compliance review
- Update security policies
- Security training
- Disaster recovery test

---

## Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [Docker Security Best Practices](https://docs.docker.com/engine/security/)
- [PostgreSQL Security](https://www.postgresql.org/docs/current/security.html)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework)

---

**Last Updated:** 2026-02-26  
**Next Review:** 2026-03-26  
**Status:** ✅ Ready for production deployment with security hardening complete
