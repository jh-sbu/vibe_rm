; autotest27's follow-ups in WarehouseFurniture. The player gets 1000 extra
; health here and loses it on leaving.

String Function TestEnterWarehouse()
    GoWarehouse()
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 1000.0)
    return "in the warehouse: interior " + Player.IsInInterior() + ", WhCenter distance " + Num(Player.GetDistance(WhCenter)) + "\n"
EndFunction

; disarm: the level cap, cast mid-swing (autotest27: only an attacking
; target is disarmed; its level 15 casts were on idle NPCs). A fighter of
; base b fighting a free opponent; the cast once it swings.
String Function DisarmSwing(ActorBase b, Spell s, String label)
    Actor a = SpawnWh(b, 0.0, 0.0)
    Actor t = SpawnWh(LowFighterBase, 250.0, 0.0)
    Tough(a)
    Tough(t)
    Player.MoveTo(a, 0.0, -400.0, 0.0, false)
    Utility.Wait(1.0)
    Fight(a, t)
    Float e = Engage(a, t, 10.0)
    Float start = Now()
    Bool found = false
    While !found && Now() - start < 10.0
        If a.GetAnimationVariableBool("IsAttacking") && a.GetDistance(t) < 200.0
            found = true
        Else
            Utility.Wait(0.05)
        EndIf
    EndWhile
    (a as VrmAt21ActorLog).Take()
    Bool attacking = a.GetAnimationVariableBool("IsAttacking")
    String r = "  " + label + " (level " + a.GetLevel() + ", engaged after " + Num(e) + " s, waited " + Num(Now() - start) + " s): attacking " + attacking + ", holds " + Id(a.GetEquippedWeapon(false))
    s.Cast(Player, a)
    Utility.Wait(2.0)
    r += "; after: holds " + Id(a.GetEquippedWeapon(false)) + ", events " + (a as VrmAt21ActorLog).Take() + "\n"
    a.StopCombat()
    t.StopCombat()
    Gone(a)
    Gone(t)
    SweepWh(IronSword)
    Reset()
    return r
EndFunction

String Function TestDisarmCap()
    String r = "our Disarm cast mid-swing:\n"
    r += DisarmSwing(LowFighterLoggedBase, OurDisarm, "12 on level 1 (control)")
    r += DisarmSwing(Level15FighterBase, OurDisarm, "12 on level 15, 1")
    r += DisarmSwing(Level15FighterBase, OurDisarm, "12 on level 15, 2")
    r += DisarmSwing(Level15FighterBase, OurDisarm, "12 on level 15, 3")
    r += DisarmSwing(Level15FighterBase, OurDisarmStrong, "30 on level 15, 1")
    r += DisarmSwing(Level15FighterBase, OurDisarmStrong, "30 on level 15, 2")
    r += DisarmSwing(Level15FighterBase, OurDisarmStrong, "30 on level 15, 3")
    return r
EndFunction

; npc-healing: the Vigilant with 600 health (autotest27's had 90 and died
; within 20 s), HealRate 0, free, fighting an attacker with 5000 health
; from a given health fraction; health fraction/magicka/state every 4 s.
String Function HealerFrom(ActorBase attacker, Float fraction, Float seconds, String label)
    Actor h = SpawnWh(VigilantLogBase, 0.0, 0.0)
    Actor a = SpawnWh(attacker, 300.0, 0.0)
    Tough(a)
    h.SetActorValue("Health", 600.0)
    h.RestoreActorValue("Health", 10000.0)
    h.ForceActorValue("HealRate", 0.0)
    h.RestoreActorValue("Magicka", 1000.0)
    Player.MoveTo(WhCenter, -700.0, 0.0, 0.0, false)
    Utility.Wait(1.5)
    h.DamageActorValue("Health", h.GetActorValueMax("Health") * (1.0 - fraction))
    (h as VrmAt21ActorLog).Take()
    Fight(a, h)
    String r = "  " + label + " from " + Num(h.GetActorValuePercentage("Health")) + " of " + Num(h.GetActorValueMax("Health")) + ":"
    Float start = Now()
    While Now() - start < seconds
        Utility.Wait(4.0)
        r += " " + Num(Now() - start) + ":" + Num(h.GetActorValuePercentage("Health")) + "/" + Num(h.GetActorValue("Magicka")) + "/" + h.GetCombatState()
    EndWhile
    r += "\n    events " + (h as VrmAt21ActorLog).Take() + "\n"
    h.StopCombat()
    a.StopCombat()
    Gone(h)
    Gone(a)
    SweepWh(IronSword)
    Reset()
    return r
EndFunction

String Function TestHealer()
    String r = "a Vigilant (600 health) against a level 1 fighter with a sword; health fraction/magicka/state every 4 s:\n"
    r += HealerFrom(LowFighterBase, 0.7, 24.0, "melee")
    r += HealerFrom(LowFighterBase, 0.5, 24.0, "melee")
    r += HealerFrom(LowFighterBase, 0.3, 24.0, "melee")
    return r
EndFunction

String Function TestHealerWarlock()
    String r = "a Vigilant (600 health) against a fire warlock; health fraction/magicka/state every 4 s:\n"
    r += HealerFrom(WarlockLogBase, 0.7, 28.0, "fire")
    r += HealerFrom(WarlockLogBase, 0.4, 28.0, "fire")
    return r
EndFunction

; detection: how long an unalerted NPC's scripted draw lasts (autotest27's
; loop gave up before the weapon was out). Polled every 0.05 s for 10 s:
; when IsWeaponDrawn first reads true, and when false again; with `again`,
; DrawWeapon is called again that long after it first reads drawn.
String Function SheatheTime(Float again)
    Actor a = SpawnWh(LowLoggedBase, 0.0, 300.0)
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    Utility.Wait(1.5)
    String r = "alerted " + a.IsAlerted() + ", drawn " + a.IsWeaponDrawn()
    a.DrawWeapon()
    Float t0 = Now()
    Float drawn = -1.0
    Float sheathed = -1.0
    Float redrawn = -1.0
    While sheathed < 0.0 && Now() - t0 < 10.0
        Bool d = a.IsWeaponDrawn()
        If drawn < 0.0 && d
            drawn = Now() - t0
        ElseIf drawn >= 0.0 && !d
            sheathed = Now() - t0
        EndIf
        If again > 0.0 && drawn >= 0.0 && redrawn < 0.0 && Now() - t0 >= drawn + again
            a.DrawWeapon()
            redrawn = Now() - t0
        EndIf
        Utility.Wait(0.05)
    EndWhile
    r += "; DrawWeapon at 0: drawn at " + Num(drawn)
    If redrawn >= 0.0
        r += ", DrawWeapon again at " + Num(redrawn)
    EndIf
    r += ", sheathed at " + Num(sheathed) + " (-1: not within 10 s)\n"
    a.SetAlert(false)
    Gone(a)
    SweepWh(IronSword)
    return r
EndFunction

String Function TestSheatheDelay()
    Player.MoveTo(WhCenter, -300.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    String r = "an unalerted NPC with a sword, free, 300 off:\n"
    r += "  once: " + SheatheTime(-1.0)
    r += "  once, 2: " + SheatheTime(-1.0)
    r += "  once, 3: " + SheatheTime(-1.0)
    r += "  again 0.5 s after drawn: " + SheatheTime(0.5)
    return r + "  again 0.5 s after drawn, 2: " + SheatheTime(0.5)
EndFunction

; crime: Attack on Sight above 1000 (autotest27: no attack at 1000 in
; 40 s). Our IsGuard guard of our Attack on Sight faction, 500 off, at
; 2000 and then 5000, 25 s each.
String Function TestAttackOnSightHigh()
    Actor g = SpawnWh(AosIsGuardBase, 500.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(3.0)
    String r = "a guard (IsGuard " + g.IsGuard() + ") of our faction with Arrest and Attack on Sight: bounty 0, " + Views(g) + "\n"
    Int[] bounty = new Int[2]
    bounty[0] = 2000
    bounty[1] = 5000
    Int i = 0
    While i < 2
        AosFaction.SetCrimeGold(bounty[i])
        Float start = Now()
        While Now() - start < 25.0
            Utility.Wait(5.0)
            r += "  bounty " + bounty[i] + " +" + Num(Now() - start) + " s: " + Views(g) + "\n"
        EndWhile
        i += 1
    EndWhile
    AosFaction.SetCrimeGold(0)
    g.StopCombat()
    Gone(g)
    Reset()
    return r
EndFunction

; magic: whom Fury sends its target at (autotest27: the caster, 139 off,
; nearer than the other NPC at 200). Here the other is nearer: 150 off,
; the caster 450.
String Function TestFuryTarget()
    Actor f = SpawnWh(LowLoggedBase, 0.0, 0.0)
    Actor n = SpawnWh(NakedBase, 150.0, 0.0)
    n.SetDontMove(true)
    Player.MoveTo(f, 0.0, -450.0, 0.0, false)
    Utility.Wait(1.5)
    (f as VrmAt21ActorLog).Take()
    String r = "our Fury (6) on a level 1 unaggressive NPC, the other 150 off, the player " + Num(Player.GetDistance(f)) + " off: state/target/aggression every 2 s:"
    OurFury.Cast(Player, f)
    Int s = 0
    While s < 4
        Utility.Wait(2.0)
        r += " " + f.GetCombatState() + "/" + TargetOf(f, n) + "/" + Num(f.GetActorValue("Aggression"))
        s += 1
    EndWhile
    r += "; the other's health lost " + Num(n.GetActorValueMax("Health") - Health(n)) + ", events " + (f as VrmAt21ActorLog).Take() + "\n"
    f.StopCombat()
    n.StopCombat()
    Gone(f)
    Gone(n)
    Reset()
    return r
EndFunction

; actor holds: SetRestrained on a fighter already fighting (autotest27: a
; restrained one given StartCombat stayed out of combat). It is engaged,
; then restrained; state, swings and damage for 10 s.
String Function TestRestrainedMidFight()
    Actor s = SpawnWh(LowFighterLoggedBase, 0.0, 0.0)
    Actor e = SpawnWh(LowFighterBase, 150.0, 0.0)
    Tough(s)
    Tough(e)
    Player.MoveTo(WhCenter, -600.0, 0.0, 0.0, false)
    Utility.Wait(1.5)
    Fight(s, e)
    Float g = Engage(s, e, 10.0)
    Utility.Wait(1.0)
    Float dealt = 5000.0 - Health(e)
    Float x = s.GetPositionX()
    Float y = s.GetPositionY()
    String r = "engaged after " + Num(g) + " s, state " + s.GetCombatState() + "; SetRestrained(true); every 2 s state/drawn:"
    s.SetRestrained(true)
    Int swings = 0
    Bool was = false
    Float start = Now()
    Float next = 2.0
    While Now() - start < 10.0
        Bool atk = s.GetAnimationVariableBool("IsAttacking")
        If atk && !was
            swings += 1
        EndIf
        was = atk
        If Now() - start >= next
            r += " " + s.GetCombatState() + "/" + s.IsWeaponDrawn()
            next += 2.0
        EndIf
        Utility.Wait(0.1)
    EndWhile
    Float dx = s.GetPositionX() - x
    Float dy = s.GetPositionY() - y
    r += "\n  restrained 10 s: swings " + swings + ", damage dealt " + Num(5000.0 - Health(e) - dealt) + ", taken " + Num(5000.0 - Health(s)) + ", moved " + Num(Math.Sqrt(dx * dx + dy * dy)) + "\n"
    s.SetRestrained(false)
    s.StopCombat()
    e.StopCombat()
    Gone(s)
    Gone(e)
    SweepWh(IronSword)
    Reset()
    return r
EndFunction

; magic: a lawful NPC held by SetDontMove, hit by a script cast of the
; player's fire spell (autotest21's, held so, searched: state 2;
; autotest27's free one fought).
String Function TestHostileSpellHeld()
    Actor a = SpawnWh(LoggedBase, 0.0, 400.0)
    a.SetCrimeFaction(CrimeFaction)
    a.SetDontMove(true)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(1.5)
    (a as VrmAt21ActorLog).Take()
    CrimeFaction.SetCrimeGold(0)
    WhFireSpell.Cast(Player, a)
    String r = "a fire spell cast by script from the player at a lawful NPC held by SetDontMove 400 off; state/targets the player at:"
    Float start = Now()
    Int i = 0
    While i < 4
        Utility.Wait(2.0)
        r += " " + Num(Now() - start) + "s " + a.GetCombatState() + "/" + (a.GetCombatTarget() == Player)
        i += 1
    EndWhile
    r += "; crime gold " + CrimeFaction.GetCrimeGold() + ", events " + (a as VrmAt21ActorLog).Take() + "\n"
    a.SetDontMove(false)
    a.StopCombat()
    CrimeFaction.SetCrimeGold(0)
    Gone(a)
    Reset()
    return r
EndFunction

String Function TestLeaveWarehouse()
    Player.ModActorValue("Health", -1000.0)
    GoCell()
    return "back in the test cell\n"
EndFunction
