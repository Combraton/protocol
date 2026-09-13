"""Fixture sensitivity: which deliberate deviations does the suite notice?

Copies this implementation to a temporary directory, applies one deviation at
a time (one or more source edits, each breaking a behavior the documents
require or a choice recorded in DIVERGENCES.md), runs the black-box
conformance runner against the copy and reports which fixtures fail. A
deviation with 0 newly failing fixtures (compared with the unmodified
implementation, run first) is a behavior no fixture checks.

Run from the repository root after `cargo build -p combraton-conformance --locked`:

    python3 conformance/independent/python-core/tests/fixture_sensitivity.py [m1|m2|f] [--check] [--jobs N]

Without a group all groups run. `--check` only verifies that every edit
applies to the current sources. Variants run N at a time (default 4; about
10 s of suite time each). Groups: m1 = section D, m2 = section E.4, f =
section F (the requirements resolved in M2-DIVERGENCES section E).
"""
import concurrent.futures, json, os, shutil, subprocess, sys, tempfile

REPO = os.getcwd()
SCR = tempfile.mkdtemp(prefix="indep-py-sensitivity-")
IMPL = os.path.join(REPO, "conformance/independent/python-core")
P, EV, G, ST, EN, VD = "provider.py", "events.py", "grants.py", "state.py", "envelope.py", "valuedomain.py"

# (group, name, [(file, old, new), ...])
V = [
 # ------------------------------------------------------------------ M1 (section D)
 ("m1", "binding-level errors use retry same_command", [(P, '"parse_error": "no", "invalid_utf8": "no", "frame_too_large": "no", "invalid_request": "no"', '"parse_error": "same_command", "invalid_utf8": "same_command", "frame_too_large": "same_command", "invalid_request": "same_command"')]),
 ("m1", "exit status 3 at end of input", [(P, "    serve(provider)\n    return 0", "    serve(provider)\n    return 3")]),
 ("m1", "limits off-by-one (>= instead of >)", [(P, "            if observed > lim[name]:", "            if observed >= lim[name]:")]),
 ("m1", "max_payload_bytes ignored", [(P, 'if "payload" in params and len(V.canonical(params["payload"])) > lim["max_payload_bytes"]:', 'if False:')]),
 ("m1", "max_array_items ignored", [(P, '            ("max_array_items", longest_array),\n', '')]),
 ("m1", "frame limit never raised by negotiation", [(P, '        s.recv_limit = self.limits["max_frame_bytes"]\n', '')]),
 ("m1", "precondition_failed lists only first failure", [(P, '                failed.append(item)', '                failed.append(item); break')]),
 ("m1", "epoch checked after preconditions", [(P, '        current_epoch = self.epoch_of(AUTHORITY_SCOPE)\n', '        self.check_preconditions(env, auth)\n        current_epoch = self.epoch_of(AUTHORITY_SCOPE)\n')]),
 ("m1", "noncharacters accepted", [(VD, '    if _NONCHAR.search(s):', '    if False:')]),
 ("m1", "lone surrogate escapes accepted", [(VD, '                s = s.encode("utf-16-le", "surrogatepass").decode("utf-16-le")', '                s = s.encode("utf-16-le", "surrogatepass").decode("utf-16-le", "surrogatepass")')]),
 ("m1", "-0 accepted", [(VD, '            if tok == "-0":', '            if False:')]),
 ("m1", "failed negotiation blocks retry (already_negotiated)", [(P, '        if s.negotiated:\n            raise ProtocolError("already_negotiated")\n', '        if getattr(s, "tried", False):\n            raise ProtocolError("already_negotiated")\n        s.tried = True\n')]),
 ("m1", "requires uniqueness not enforced", [(EN, 'req = array(env["requires"], "/requires", max_items=64, unique=True)', 'req = array(env["requires"], "/requires", max_items=64)')]),
 ("m1", "query requires not checked", [(P, '            item for item in params.get("requires", [])\n', '            item for item in (params.get("requires", []) if kind == "command" else [])\n')]),
 ("m1", "primary-subject precondition not mandatory", [(EN, '    if not any(e["subject"] == env["subject"] for e in env["preconditions"]):', '    if False:')]),
 ("m1", "duplicate precondition subjects accepted", [(EN, '        if key in seen_subjects:\n', '        if False:\n')]),
 ("m1", "unknown method before negotiation -> negotiation_required", [(P, '        if op is None:\n            raise ProtocolError("method_not_found", {"operation": method})', '        if op is None:\n            raise ProtocolError("method_not_found" if self.session.negotiated else "negotiation_required", {"operation": method})')]),
 ("m1", "unknown request members accepted", [(P, '            or set(msg) - {"jsonrpc", "id", "method", "params"}\n', '')]),
 ("m1", "non-null invalid ids accepted (bool/empty)", [(P, '    if isinstance(rid, bool):\n        return False', '    if isinstance(rid, bool):\n        return True')]),
 ("m1", "negotiation code precedence reversed", [(P, '            if reasons & {"unknown_profile", "declared_unsupported", "dependency_not_selected"}:\n                code = "unsupported_profile"\n            elif "no_common_major" in reasons:\n                code = "unsupported_version"', '            if "no_common_major" in reasons:\n                code = "unsupported_version"\n            elif reasons & {"unknown_profile", "declared_unsupported", "dependency_not_selected"}:\n                code = "unsupported_profile"')]),
 ("m1", "correlation included in digest intent", [(P, '            "extensions": {k: v for k, v in extensions.items() if k in requires},\n        }', '            "extensions": {k: v for k, v in extensions.items() if k in requires},\n        }\n        if "correlation" in env: intent["correlation"] = env["correlation"]')]),
 ("m1", "optional feature selected even if unsupported", [(P, '                if f in support["features"]:', '                if True:')]),
 ("m1", "records discarded below current regardless of retain", [(ST, '            self.db.execute("DELETE FROM commands WHERE generation < ?", (oldest,))', '            self.db.execute("DELETE FROM commands WHERE generation < ?", (current,))')]),

 # ------------------------------------------------------------ M2 grants (CORE 15)
 ("m2", "[control] revocation does not cascade", [(P, '            queue.extend(children.get(gid, []))', '            pass')]),
 ("m2", "[control] non-authority may issue root grants", [(P, '            if not self.is_authority:\n                raise denied("not_authority")', '            if False:\n                raise denied("not_authority")')]),
 ("m2", "[control] grant expiry ignored", [(G, '    if "expires_at" in record and now >= record["expires_at"]:', '    if False:')]),
 ("m2", "[control] other precondition subjects need no read right", [(P, '            needs += [("core-test.read", p["subject"]) for p in env["preconditions"] if p["subject"] != env["subject"]]\n', '')]),
 ("m2", "delegation: parent's delegation.allowed ignored", [(G, '    if not pd["allowed"] or pd["max_depth"] < 1:', '    if pd["max_depth"] < 1:')]),
 ("m2", "delegation: child may expire after its parent", [(G, '    if "expires_at" in parent and ("expires_at" not in child or child["expires_at"] > parent["expires_at"]):', '    if False:')]),
 ("m2", "delegation: authority_binding not inherited", [(G, '    if "authority_binding" in parent and child.get("authority_binding") != parent["authority_binding"]:', '    if False:')]),
 ("m2", "delegation: revoked/expired/stale parent may still delegate", [(P, '        problem = G.usable_problem(parent, self.now(), self.epoch_of)', '        problem = None')]),
 ("m2", "delegation: any principal may delegate from a parent", [(P, '        if row is None or row[1]["holder"] != self.principal:\n            raise denied("grant_not_found")\n        parent = row[1]', '        if row is None:\n            raise denied("grant_not_found")\n        parent = row[1]')]),
 ("m2", "revoke: any principal may revoke", [(P, '        if not self.is_authority and issuer != self.principal:', '        if False:')]),
 ("m2", "grant.get hidden from a non-authority issuer", [(P, 'self.principal in (row[1]["holder"], row[1]["issuer"])):', 'self.principal == row[1]["holder"]):')]),
 ("m2", "expiry boundary: grant still usable at exactly expires_at", [(G, 'now >= record["expires_at"]', 'now > record["expires_at"]')]),
 ("m2", "issue: expires_at equal to the provider clock accepted", [(P, 'terms["expires_at"] <= self.now()', 'terms["expires_at"] < self.now()')]),
 ("m2", "issue: expiry validated before deduplication (replay refused)", [(P, '            # Step 5: deduplication lookup (CORE 6.3 table, in order).\n', '            if method == "core.grant.issue" and env["payload"].get("expires_at", "9999") <= self.now():\n                raise ProtocolError("invalid_envelope", {"path": "/payload/expires_at", "reason": "past"})\n')]),
 ("m2", "grant state checked before holder (revoked grant of another principal says revoked)", [(P, '        if row is None or row[1]["holder"] != self.principal:\n            raise denied("grant_not_found")\n        record = row[1]\n        problem = G.usable_problem(record, self.now(), self.epoch_of)\n        if problem:\n            raise denied(problem)', '        if row is None:\n            raise denied("grant_not_found")\n        record = row[1]\n        problem = G.usable_problem(record, self.now(), self.epoch_of)\n        if problem:\n            raise denied(problem)\n        if record["holder"] != self.principal:\n            raise denied("grant_not_found")')]),
 ("m2", "authority acting under a named grant is not restricted by it", [(P, '        if grant_id is None:\n            if self.is_authority:', '        if grant_id is None or self.is_authority:\n            if self.is_authority:')]),
 ("m2", "claim needs no core-test.claim right", [(P, 'return self.authorize_under(env, [("core-test.claim", E.AUTHORITY_SUBJECT)])', 'return self.authorize_under(env, [])')]),
 ("m2", "applied_count not protected", [(P, '        if method in ("core-test.subject.get", "core-test.subject.applied_count"):', '        if method == "core-test.subject.get":')]),
 ("m2", "precondition current revealed to principals that may not read", [(P, '                if auth.may_read(subj):', '                if True:')]),
 ("m2", "grant field on a non-grants session accepted", [(EN, 'QUERY_OPTIONAL + (GRANT_FIELD if allow_grant else ())', 'QUERY_OPTIONAL + GRANT_FIELD'), (EN, 'COMMAND_OPTIONAL + (GRANT_FIELD if allow_grant else ())', 'COMMAND_OPTIONAL + GRANT_FIELD')]),
 ("m2", "denial order: scope before rights", [(P, '        if any(right not in record["rights"] for right, _ in needs):\n            raise denied("right_missing")\n        if any(subj is not None and not G.grant_covers(record, subj) for _, subj in needs):\n            raise denied("out_of_scope")', '        if any(subj is not None and not G.grant_covers(record, subj) for _, subj in needs):\n            raise denied("out_of_scope")\n        if any(right not in record["rights"] for right, _ in needs):\n            raise denied("right_missing")')]),
 ("m2", "core.grant.issued events not recorded", [(P, '        return 1, {"grant": record}, [("core.grant.issued", env["subject"], 1, {"grant": record})]', '        return 1, {"grant": record}, []')]),
 ("m2", "core.grant.revoked event only for the revoked root", [(P, '            changes.append(("core.grant.revoked", {"kind": E.GRANT_KIND, "id": gid}, rev, {"state": "revoked"}))', '            changes += [("core.grant.revoked", {"kind": E.GRANT_KIND, "id": gid}, rev, {"state": "revoked"})] if gid == root else []')]),

 # ------------------------------------------------------------ M2 events (CORE 16)
 ("m2", "[control] caused_by not copied", [(P, '                    "caused_by": list(env.get("caused_by", [])),', '                    "caused_by": [],')]),
 ("m2", "gap snapshot and to at the stream head (retained events folded into the gap)", [(EV, '            through, snapshot = disc, store.base_snapshot()\n', '            import state as S_, valuedomain as V_\n            through, merged = store.head(), {(s_["subject"]["kind"], s_["subject"]["id"]): s_ for s_ in store.base_snapshot()}\n            for (body_,) in store.db.execute("SELECT body FROM events ORDER BY epoch, sequence").fetchall():\n                ev_ = V_.loads(body_); k_ = (ev_["subject"]["kind"], ev_["subject"]["id"])\n                merged[k_] = {"subject": ev_["subject"], "revision": ev_["revision"], "state": S_.reduce_state(merged[k_]["state"] if k_ in merged else None, ev_)}\n            snapshot = [merged[k_] for k_ in sorted(merged)]\n')]),
 ("m2", "filtered always true under a grant (the E-FILTERED choice, since resolved)", [(P, '                "filtered": hidden,', '                "filtered": hidden or auth.grant is not None,')]),
 ("m2", "cursor from another stream accepted", [(EV, '    if m.group(1) != store.stream_id():\n        raise CursorError("other_stream")', '    if False:\n        raise CursorError("other_stream")')]),
 ("m2", "cursor beyond the head accepted", [(EV, '    if pos > store.head():\n        raise CursorError("beyond_end")', '    if False:\n        raise CursorError("beyond_end")')]),
 ("m2", "notifications sent before the command's response", [(P, '        self.write_frame(response)\n        # CORE 16.5: notifications for events a command caused go out after\n        # that command\'s response; a new subscription\'s backlog after its own.\n        self.deliver_notifications()', '        self.deliver_notifications()\n        self.write_frame(response)')]),
 ("m2", "subscription ignores kinds", [(P, 'EV.read_items(self.store, start, n, auth.sees, sub["kinds"])', 'EV.read_items(self.store, start, n, auth.sees, None)')]),
 ("m2", "subscription ignores grant resources", [(P, 'EV.read_items(self.store, start, n, auth.sees, sub["kinds"])', 'EV.read_items(self.store, start, n, lambda s: True, sub["kinds"])')]),
 ("m2", "subscribe needs no authorization", [(P, '        if method in ("core.events.read", "core.events.subscribe"):', '        if method == "core.events.read":')]),
 ("m2", "events.read under any grant without core.events.read", [(P, 'return self.authorize_under(env, [("core.events.read", None)])', 'return self.authorize_under(env, [])')]),
 ("m2", "gap snapshot not filtered by authorization", [(EV, '            subjects = [s for s in snapshot if shown(s["subject"])]', '            subjects = snapshot')]),
 ("m2", "[control] epoch_change item omitted", [(EV, '            items.append({"epoch_change": {"from_epoch": epoch, "to_epoch": epoch + 1,\n                                           "vouched_through": store.vouched_through(epoch)}})\n', '')]),
 ("m2", "[control] unsubscribe does not stop delivery", [(P, '        if self.session.subscriptions.pop(env["payload"]["subscription"], None) is None:', '        if env["payload"]["subscription"] not in self.session.subscriptions:')]),

 # ------------------------------------------------------ M2 capabilities (CORE 17)
 ("m2", "[control] initial snapshot appends an event", [(ST, '            if revision > 1:', '            if True:')]),
 ("m2", "capability checked after the authority epoch and preconditions", [(P, '            self.check_capabilities(method)\n', ''), (P, '        self.check_preconditions(env, auth)\n        subj = env["subject"]\n        revision = self.store.revision(subj["kind"], subj["id"]) + 1', '        self.check_preconditions(env, auth)\n        self.check_capabilities("core-test.subject.put")\n        subj = env["subject"]\n        revision = self.store.revision(subj["kind"], subj["id"]) + 1')]),
 ("m2", "capability checked before authorization", [(P, '            auth = self.authorize(method, env)\n            # Step 7, first part', '            self.check_capabilities(method)\n            auth = self.authorize(method, env)\n            # Step 7, first part')]),
 ("m2", "revision raised only when a status changes (evidence ignored)", [(ST, '            if revision and meaning(stored) == meaning(predicates):', '            if revision and [p["status"] for p in stored] == [p["status"] for p in predicates]:')]),
 ("m2", "capability event revision differs from the snapshot revision", [(P, '            "subject": {"kind": "core.capabilities", "id": config["provider_id"]},\n            "revision": revision,', '            "subject": {"kind": "core.capabilities", "id": config["provider_id"]},\n            "revision": revision + 100,')]),
 ("m2", "capability event subject id is not provider_id", [(P, '"id": config["provider_id"]},', '"id": "capabilities"},')]),
 ("m2", "claim also depends on core-test.writes", [(P, 'OPERATION_CAPABILITIES = {"core-test.subject.put": ("core-test.writes",)}', 'OPERATION_CAPABILITIES = {"core-test.subject.put": ("core-test.writes",), "core-test.authority.claim": ("core-test.writes",)}')]),

 # ------------------------------------------ F: grants and preconditions (CORE 7, 15)
 ("f", "[control] authority_binding to an unknown scope accepted", [(P, '        if "authority_binding" in terms and terms["authority_binding"]["scope"] not in TRACKED_SCOPES:', '        if False:')]),
 ("f", "authority_binding to a later epoch of a known scope refused at issue", [(P, '        if "authority_binding" in terms and terms["authority_binding"]["scope"] not in TRACKED_SCOPES:', '        if "authority_binding" in terms and (terms["authority_binding"]["scope"] not in TRACKED_SCOPES or terms["authority_binding"]["epoch"] > self.epoch_of(AUTHORITY_SCOPE)):')]),
 ("f", "binding scope checked before audience and expiry", [(P, '        if terms["audience"] != self.provider_id:', '        if "authority_binding" in terms and terms["authority_binding"]["scope"] not in TRACKED_SCOPES:\n            raise ProtocolError("invalid_envelope", {"path": "/payload/authority_binding/scope", "reason": "x"})\n        if terms["audience"] != self.provider_id:')]),
 ("f", "grant precondition revision unchecked (issue revision != 0, revoke revision 0)", [(EN, '    if _single_primary_precondition(env)["revision"] != 0:', '    if _single_primary_precondition(env) is None:'), (EN, '    if _single_primary_precondition(env)["revision"] == 0:', '    if _single_primary_precondition(env) is None:')]),
 ("f", "re-revocation: revoked reported before not_authority (a stranger learns the state)", [(P, '        if not self.is_authority and issuer != self.principal:\n            # CORE 15.3: "whether or not the grant exists".\n            raise denied("not_authority")\n        if row is not None and row[1]["state"] == "revoked":\n            # CORE 15.3: re-revocation, "decided after that check".\n            raise denied("revoked")', '        if row is not None and row[1]["state"] == "revoked":\n            raise denied("revoked")\n        if not self.is_authority and issuer != self.principal:\n            raise denied("not_authority")')]),
 ("f", "re-revocation accepted (new revision and event for the named grant)", [(P, '            # CORE 15.3: re-revocation, "decided after that check".\n            raise denied("revoked")', '            pass'), (P, '            if record["state"] == "revoked":\n                continue', '            if record["state"] == "revoked" and gid != root:\n                continue')]),
 ("f", "re-revocation checked after preconditions (stale revision gives precondition_failed)", [(P, '            # CORE 15.3: re-revocation, "decided after that check".\n            raise denied("revoked")', '            pass'), (P, '        self.check_preconditions(env, auth)\n        root = env["subject"]["id"]', '        self.check_preconditions(env, auth)\n        root = env["subject"]["id"]\n        if self.store.grant(root)[1]["state"] == "revoked":\n            raise denied("revoked")')]),
 ("f", "revocation lists and re-revokes descendants that were already revoked", [(P, '            if record["state"] == "revoked":\n                continue', '            if record["state"] == "revoked" and gid == root:\n                continue')]),
 ("f", "[control] current omitted for an authority acting without a grant", [(P, '            return True  # "An authority principal acting without a grant may read every subject."', '            return False')]),
 ("f", "current under a grant needs only resource coverage, not the read right", [(P, '            return self.grant_shows(subject)\n        if self.provider.is_authority:', '            return G.grant_covers(self.grant, subject)\n        if self.provider.is_authority:')]),
 ("f", "current omitted for a delegating non-authority's own grants (F-PRE-CURRENT-NO-GRANT reversed)", [(P, '        return subject["kind"] == E.GRANT_KIND and self._holder_or_issuer(subject["id"])', '        return False')]),
 ("f", "stale_authority_epoch.current_epoch always disclosed", [(P, '            details = {"current_epoch": current_epoch} if auth.may_read(E.AUTHORITY_SUBJECT) else {}', '            details = {"current_epoch": current_epoch}')]),
 ("f", "core.capabilities protected (non-authority needs a grant)", [(P, '        if method == "core.grant.issue":\n            return self.authorize_issue(env)', '        if method == "core.capabilities":\n            return self.authorize_under(env, [])\n        if method == "core.grant.issue":\n            return self.authorize_issue(env)')]),
 ("f", "core.events.unsubscribe protected (non-authority needs a grant)", [(P, '        if method == "core.grant.issue":\n            return self.authorize_issue(env)', '        if method == "core.events.unsubscribe":\n            return self.authorize_under(env, [])\n        if method == "core.grant.issue":\n            return self.authorize_issue(env)')]),
 ("f", "grant field evaluated on unprotected queries and core.grant.get", [(P, '        if method == "core.grant.issue":\n            return self.authorize_issue(env)', '        if method in ("core.capabilities", "core.events.unsubscribe", "core.describe", "core.grant.get") and "grant" in env:\n            return self.authorize_under(env, [])\n        if method == "core.grant.issue":\n            return self.authorize_issue(env)')]),
 ("f", "authorization skipped when core.grants was not negotiated", [(P, '            if self.is_authority:\n                return Auth(self, None)\n            raise denied("grant_required")', '            if self.is_authority or not self.session.feature_selected("core.grants"):\n                return Auth(self, None)\n            raise denied("grant_required")')]),

 # ----------------------------------------------- F: event visibility (CORE 16.6)
 ("f", "[control] profile subjects visible with resource coverage alone (no read right)", [(P, '        return right is not None and right in self.grant["rights"] and G.grant_covers(self.grant, subject)', '        return G.grant_covers(self.grant, subject)')]),
 ("f", "[control] core.grant subjects visible only with resources covering core.grant", [(P, '            return self._holder_or_issuer(subject["id"])\n        if kind == CAPABILITIES_KIND:', '            return G.grant_covers(self.grant, subject)\n        if kind == CAPABILITIES_KIND:')]),
 ("f", "every core.grant subject visible under any events grant", [(P, '            return self._holder_or_issuer(subject["id"])\n        if kind == CAPABILITIES_KIND:', '            return True\n        if kind == CAPABILITIES_KIND:')]),
 ("f", "core.grant subjects visible to the holder only, not the issuer", [(P, '            return self._holder_or_issuer(subject["id"])\n        if kind == CAPABILITIES_KIND:', '            row_ = self.provider.store.grant(subject["id"])\n            return row_ is not None and row_[1]["holder"] == self.provider.principal\n        if kind == CAPABILITIES_KIND:')]),
 ("f", "core-test.authority visible with core-test.read but no resource covering it", [(P, '        right = PROFILE_READ_RIGHTS.get(kind)\n', '        if kind == "core-test.authority":\n            return "core-test.read" in self.grant["rights"]\n        right = PROFILE_READ_RIGHTS.get(kind)\n')]),
 ("f", "core.capabilities visible under any events grant", [(P, '        if kind == CAPABILITIES_KIND:\n            return G.grant_covers(self.grant, subject)', '        if kind == CAPABILITIES_KIND:\n            return True')]),
 ("f", "core.capabilities also needs core-test.read", [(P, '        if kind == CAPABILITIES_KIND:\n            return G.grant_covers(self.grant, subject)', '        if kind == CAPABILITIES_KIND:\n            return "core-test.read" in self.grant["rights"] and G.grant_covers(self.grant, subject)')]),
 ("f", "an authority reading under a grant sees every event", [(P, '        return self.grant is None or self.grant_shows(subject)', '        return self.grant is None or self.provider.is_authority or self.grant_shows(subject)')]),

 # ------------------------------------------ F: filtered, cursors, snapshots (CORE 16.4)
 ("f", "filtered ignores events hidden only by kinds", [(EV, '        if shown(event["subject"]):\n            items.append({"event": event})\n        else:\n            hidden = True', '        if shown(event["subject"]):\n            items.append({"event": event})\n        else:\n            hidden = hidden or not visible(event["subject"])')]),
 ("f", "filtered ignores hidden snapshot subjects", [(EV, '            if len(subjects) != len(snapshot):\n                hidden = True', '            if False:\n                hidden = True')]),
 ("f", "filtered counts hidden events beyond the covered range", [(P, '                "filtered": hidden,', '                "filtered": EV.read_items(self.store, start, 10**9, auth.sees, payload.get("kinds"))[2],')]),
 ("f", "next_cursor stops at the last item although trailing hidden events were covered", [(EV, '    items: list[dict] = []\n    hidden = False\n', '    items: list[dict] = []\n    hidden = False\n    last = pos\n'), (EV, '            items.append({"event": event})\n', '            items.append({"event": event}); last = pos\n'), (EV, '            pos = (epoch + 1, 0)\n            continue', '            pos = (epoch + 1, 0); last = pos\n            continue'), (EV, '            pos = through\n            continue', '            pos = through; last = pos\n            continue'), (EV, '    return items, pos, hidden', '    return items, last, hidden')]),
 ("f", "snapshot state of a revoked grant is the revocation payload, not {grant}", [(ST, '    if kind == "core.grant.revoked":', '    if False:')]),
 ("f", "[control] snapshot omits core.grant subjects", [(EV, '            subjects = [s for s in snapshot if shown(s["subject"])]', '            subjects = [s for s in snapshot if shown(s["subject"]) and s["subject"]["kind"] != "core.grant"]')]),

 # ------------------------------------------------ F: epochs and unvouched_last
 ("f", "[control] unvouched events still delivered before the epoch change", [(ST, '        return self.vouched_through(epoch)\n\n    def vouched_through', '        return self.last_sequence(epoch)\n\n    def vouched_through'), (ST, '                self.db.execute("DELETE FROM events WHERE epoch = ? AND sequence > ?", (epoch, vouched))\n', '')]),
 ("f", "[control] unvouched_last ignored (vouched through the last sequence)", [(ST, '                vouched = max(0, seq - unvouched_last)', '                vouched = seq')]),
 ("f", "[control] cursor in a closed epoch past vouched_through refused", [(EV, '    if pos > store.head():', '    if pos > store.head() or (pos[0] < store.head()[0] and pos[1] > store.vouched_through(pos[0])):')]),
 ("f", "cursor in a closed epoch past its last sequence refused", [(EV, '    if pos > store.head():', '    if pos > store.head() or (pos[0] < store.head()[0] and pos[1] > store.last_sequence(pos[0])):')]),
 ("f", "unvouched events count toward retain_last (kept in the stream, never delivered)", [(ST, '                self.db.execute("DELETE FROM events WHERE epoch = ? AND sequence > ?", (epoch, vouched))\n', '')]),
 ("f", "retention snapshot omits changes made by unvouched events", [(ST, '                            self._fold_into_base(ubody)\n', '                            pass\n')]),

 # ------------------------------------------------ F: size limits and subscription end
 ("f", "[control] reads ignore the caller's receive limit", [(P, '        return len(V.canonical(frame)) <= self.session.send_limit\n\n    def fit_items', '        return True\n\n    def fit_items')]),
 ("f", "a read whose first item cannot fit returns no items instead of internal_error", [(P, '        if result is None:\n            # CORE 16.4', '        if result is None:\n            return build(0)[2]\n            # CORE 16.4')]),
 ("f", "[control] notifications ignore the caller's receive limit", [(P, '                    return len(V.canonical(frame)) <= self.session.send_limit, items, (frame, pos)', '                    return True, items, (frame, pos)')]),
 ("f", "[control] item_too_large ends the subscription without a final notification", [(P, '                    self.end_subscription(sid, "item_too_large")', '                    self.session.subscriptions.pop(sid)')]),
 ("f", "item_too_large: the item is skipped and delivery continues", [(P, '                    self.end_subscription(sid, "item_too_large")\n                    break', '                    sub["pos"] = EV.read_items(self.store, start, 1, lambda s_: True, None)[1]\n                    continue')]),
 ("f", "[control] authorization loss ends the subscription without a final notification", [(P, '                self.end_subscription(sid, "authorization_lost")', '                self.session.subscriptions.pop(sid)')]),
 ("f", "authorization loss reported with reason item_too_large", [(P, '                self.end_subscription(sid, "authorization_lost")', '                self.end_subscription(sid, "item_too_large")')]),
 ("f", "final notification's next_cursor is the stream head", [(P, '                                     "next_cursor": EV.encode_cursor(self.store, sub["pos"]),', '                                     "next_cursor": EV.encode_cursor(self.store, self.store.head()),')]),
 ("f", "subscription survives revocation of its grant (only expiry and epoch end it)", [(P, '            except ProtocolError:\n                self.end_subscription(sid, "authorization_lost")\n                continue', '            except ProtocolError as exc_:\n                if exc_.details.get("reason") != "revoked":\n                    self.end_subscription(sid, "authorization_lost")\n                    continue\n                auth = Auth(self, self.store.grant(sub["env"]["grant"])[1])')]),

 # ------------------------------------------------------ F: core.authenticate (CORE 18)
 ("f", "core.authenticate unknown (method_not_found)", [(P, '            "core.authenticate": ("core", None, "query", E.authenticate_params, self.op_authenticate),\n', '')]),
 ("f", "core.authenticate needs negotiation first", [(P, '        if method not in ("core.describe", "core.negotiate", "core.authenticate"):', '        if method not in ("core.describe", "core.negotiate"):')]),
 ("f", "core.authenticate on stdio succeeds and returns the principal", [(P, '        raise ProtocolError("already_authenticated")', '        return {"principal": self.principal}')]),
 ("f", "core.authenticate on stdio answers authentication_failed", [(P, '        raise ProtocolError("already_authenticated")', '        raise ProtocolError("authentication_failed")')]),
]

args = sys.argv[1:]
check_only = "--check" in args
jobs = 4
if "--jobs" in args:
    jobs = int(args[args.index("--jobs") + 1])
groups = [a for a in args if a in ("m1", "m2", "f")]
desc = json.load(open("conformance/participants/independent-python-core.json"))
selected = [("base", "unmodified implementation (baseline)", [])] + [v for v in V if not groups or v[0] in groups]


def prepare(index, name, edits):
    vdir = os.path.join(SCR, f"variant-{index}")
    shutil.copytree(IMPL, vdir, ignore=shutil.ignore_patterns("__pycache__", "tests"))
    for fname, old, new in edits:
        p = os.path.join(vdir, fname)
        src = open(p).read()
        assert src.count(old) == 1, f"{name}: snippet not found exactly once in {fname}: {old[:60]!r}"
        open(p, "w").write(src.replace(old, new, 1))
    return vdir


def run(index, name, edits):
    vdir = prepare(index, name, edits)
    d = json.loads(json.dumps(desc)); d["launch"]["argv"][1] = os.path.join(vdir, "provider.py")
    participant = os.path.join(SCR, f"variant-{index}.json")
    json.dump(d, open(participant, "w"))
    out = subprocess.run(["./target/debug/combraton-conformance", "run", "--participant", participant,
                          "--out", os.path.join(SCR, f"variant-{index}-results")], capture_output=True, text=True)
    # Every per-fixture status other than pass and not_applicable (fail, timeout, ...) counts as failing.
    rows = [l.split() for l in out.stdout.splitlines()]
    return [r[1] for r in rows if len(r) > 1 and "." in r[1] and r[0] not in ("pass", "not_applicable", "run:")]


if check_only:
    for i, (group, name, edits) in enumerate(selected):
        prepare(i, name, edits)
    print(f"all {len(selected) - 1} deviations apply")
    shutil.rmtree(SCR, ignore_errors=True)
    sys.exit(0)

baseline = set(run(0, *selected[0][1:]))
print(f"baseline failing: {sorted(baseline) or 'none'}", flush=True)
unguarded = 0
with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
    futures = [pool.submit(run, i, name, edits) for i, (group, name, edits) in enumerate(selected) if i > 0]
    for (group, name, _), future in zip(selected[1:], futures):
        fails = future.result()
        new_fails = [f for f in fails if f not in baseline]
        fixed = sorted(baseline - set(fails))
        unguarded += not new_fails and not name.startswith("[control]")
        print(f"{len(new_fails):3d} newly failing | {group} | {name}" + (f" | {', '.join(new_fails)}" if new_fails else "")
              + (f" | now passing: {', '.join(fixed)}" if fixed else ""), flush=True)
print(f"{unguarded} non-control deviations pass every fixture")
shutil.rmtree(SCR, ignore_errors=True)
