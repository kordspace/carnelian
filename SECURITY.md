# Security Policy

## Supported Versions

The following versions of CARNELIAN are currently supported with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting Security Vulnerabilities

The CARNELIAN team takes security seriously. We appreciate your efforts to responsibly disclose your findings.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report security vulnerabilities by emailing:

📧 **security@kord.space**

Alternatively, you can contact the maintainers directly through GitHub if you have an established relationship.

### What to Include

When reporting a vulnerability, please include:

- **Description**: A clear description of the vulnerability
- **Impact**: What could an attacker achieve?
- **Reproduction Steps**: Step-by-step instructions to reproduce
- **Affected Versions**: Which versions are affected?
- **Mitigation**: Any suggested fixes or workarounds
- **Your Contact**: How can we reach you for clarifications?

### Response Timeline

We aim to respond to security reports within:

- **48 hours**: Initial acknowledgment
- **7 days**: Assessment and initial response
- **30 days**: Fix or mitigation plan
- **90 days**: Public disclosure (unless agreed otherwise)

### Security Response Process

1. **Acknowledge**: We confirm receipt within 48 hours
2. **Assess**: We evaluate the severity and impact
3. **Fix**: We develop and test a fix
4. **Notify**: We notify affected users if necessary
5. **Disclose**: We publicly disclose after fix is available

## Security Best Practices

When deploying CARNELIAN, please follow these security guidelines:

### Deployment

- [ ] Use Docker secrets or environment variables for sensitive data
- [ ] Enable HTTPS/TLS in production
- [ ] Configure proper CORS settings
- [ ] Enable rate limiting
- [ ] Use strong API keys
- [ ] Keep dependencies updated
- [ ] Enable security headers
- [ ] Configure input validation

See [SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md) for complete security hardening guide.

### Configuration

- Never commit `.env` files to version control
- Use strong, unique passwords for databases
- Enable audit logging
- Configure resource limits
- Use non-root containers

## Security Features

CARNELIAN includes the following security features:

### Authentication & Authorization
- API key-based authentication
- Capability-based security model
- Rate limiting

### Data Protection
- Input validation and sanitization
- SQL injection prevention
- XSS protection
- CSRF protection

### Network Security
- CORS configuration
- Security headers (HSTS, CSP, etc.)
- HTTPS enforcement

### Container Security
- Non-root container user support
- Read-only root filesystem support
- Secret management

## Post-Quantum Roadmap

Carnelian OS v1.0.0 ships with classical **Ed25519** cryptography as the default signing algorithm. When MAGIC quantum entropy is enabled, Ed25519 keys are seeded from quantum sources, providing high-quality randomness but not quantum resistance against future attacks.

The `carnelian-magic` crate contains **production-ready** post-quantum cryptographic primitives based on NIST-standardized algorithms:

- **HybridSigningKey**: Combines CRYSTALS-Dilithium3 (quantum-resistant) with Ed25519 (classical) for dual-signature defense-in-depth
- **KyberKem**: CRYSTALS-Kyber1024 key encapsulation mechanism for quantum-resistant key exchange

These primitives are **fully implemented and tested** but ship as an opt-in feature targeted for activation in **v1.1.0**.

### Migration Timeline

- **v1.0.0** (Current): Ed25519 + MAGIC quantum seeding
- **v1.1.0** (Q2 2026): Hybrid Dilithium3 + Ed25519 signatures (opt-in)
- **v1.2.0** (Q3 2026): Kyber1024 KEM for encryption-at-rest
- **v2.0.0** (Q4 2026): Pure post-quantum stack (Dilithium3 + Kyber1024)

### Documentation

- **Full Roadmap**: See `DOCUMENTATION/FUTURE_PQC.md` for detailed migration plan, CLI commands, and security considerations
- **Architecture Review**: See `docs/SECURITY_ARCHITECTURE_REVIEW_V1.md` for comprehensive PQC analysis
- **MAGIC Subsystem**: See `docs/MAGIC.md` for quantum entropy provider setup

## Known Security Considerations

### Current Limitations

- WASM sandboxing provides isolation but may not be perfect
- Network capabilities require careful configuration
- File system access should be limited

### Ongoing Improvements

- Regular dependency updates
- Security audits
- Penetration testing
- Vulnerability scanning

## Acknowledgments

We thank the following individuals for responsibly disclosing security issues:

*No reported vulnerabilities yet.*

## Security Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Secure Coding Guidelines](https://anssi-fr.github.io/rust-guide/)
- [Docker Security](https://docs.docker.com/engine/security/)
- [PostgreSQL Security](https://www.postgresql.org/docs/current/security.html)

## Contact

For general questions or non-security issues, please use:
- GitHub Issues: https://github.com/kordspace/carnelian/issues
- GitHub Discussions: https://github.com/kordspace/carnelian/discussions

For security issues only:
- Email: security@kord.space

---

**Last Updated**: 2026-03-03  
**Version**: 1.0
