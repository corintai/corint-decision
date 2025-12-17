when: event.amount < 100

when:
  all:
    - event.amount > 1000
    - geo.country in ["US", "CA"]
    - not:
        - risk.tags contains "proxy"

when:
  any:
    - context.total_score >= 80
    - user.is_blacklisted == true

when:
  any:

    # -------------------------------------------------------
    # A 类条件：高金额 + 高风险国家 + 非白名单
    # -------------------------------------------------------
    - all:
        - event.amount >= 3000
        - geo.country in ["NG", "PK", "UA", "RU"]
        - not:
            - user.tags contains "vip"

    # -------------------------------------------------------
    # B 类条件：设备/IP 异常 + 高频失败
    # -------------------------------------------------------
    - all:
        - any:
            - device.is_emulator == true
            - network.is_proxy == true
            - network.is_tor == true
        - risk.login_fail_count_1h >= 3

    # -------------------------------------------------------
    # C 类条件：LLM 行为推理 + 用户画像异常
    # -------------------------------------------------------
    - all:
        - llm.behavior.risk_level in ["high", "critical"]
        - any:
            - user.tags contains "abnormal_pattern"
            - user.tags contains "synthetic_identity"
            - user.tags contains "previous_fraud"