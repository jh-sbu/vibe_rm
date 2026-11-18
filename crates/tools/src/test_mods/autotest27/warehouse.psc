; autotest24's open items and the readings earlier batches took on held
; actors, in WarehouseFurniture. The player gets 1000 extra health here and
; loses it on leaving.

String Function TestEnterWarehouse()
    GoWarehouse()
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 1000.0)
    return "in the warehouse: interior " + Player.IsInInterior() + ", WhCenter distance " + Num(Player.GetDistance(WhCenter)) + "\n"
EndFunction

; Waits up to t seconds for a to be in combat (state 1) within reach of b;
; the seconds taken, or -1.
Float Function Engage(Actor a, Actor b, Float t)
    Float start = Now()
    While Now() - start < t
        If a.GetCombatState() == 1 && a.GetDistance(b) < 200.0
            return Now() - start
        EndIf
        Utility.Wait(0.25)
    EndWhile
    return -1.0
EndFunction

; Health enough that nobody dies of a test's fight.
Function Tough(Actor a)
    a.ForceActorValue("Health", 5000.0)
    a.RestoreActorValue("Health", 5000.0)
EndFunction

; Deletes every reference of weapon lying in the warehouse.
Function SweepWh(Form weapon)
    ObjectReference w = Game.FindClosestReferenceOfTypeFromRef(weapon, WhCenter, 4000.0)
    While w != None
        Gone(w)
        Utility.Wait(0.2)
        w = Game.FindClosestReferenceOfTypeFromRef(weapon, WhCenter, 4000.0)
        If w != None && w.IsDeleted()
            w = None
        EndIf
    EndWhile
EndFunction

Function Reset()
    Player.StopCombatAlarm()
    Player.MoveTo(WhCenter, -500.0, 0.0, 0.0, false)
    Player.RestoreActorValue("Health", 2000.0)
    Utility.Wait(1.0)
EndFunction

; magic: Calm, Fear and Fury as target-actor copies of the vanilla spells'
; main effects (autotest24: the vanilla Fear gave its effect event but no
; flight; Calm and Fury gave no event), cast from 150 units and read for
; 8 s. The fights are autotest24's.
String Function Near(Actor a)
    Player.MoveTo(a, 0.0, -150.0, 0.0, false)
    Utility.Wait(0.5)
    return " (player " + Num(Player.GetDistance(a)) + " off)"
EndFunction

String Function FearCase(ActorBase b, String label)
    Actor f = SpawnWh(b, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
    t.SetDontMove(true)
    f.AddItem(IronSword, 1, true)
    f.EquipItem(IronSword, false, true)
    Utility.Wait(1.0)
    f.StartCombat(t)
    Utility.Wait(3.0)
    (f as VrmAt21ActorLog).Take()
    String r = "our Fear (9) on " + label + " (level " + f.GetLevel() + ")" + Near(f) + ", combat " + f.GetCombatState() + ": distance/state/confidence every 2 s:"
    OurFear.Cast(Player, f)
    Int s = 0
    While s < 4
        Utility.Wait(2.0)
        r += " " + Num(f.GetDistance(t)) + "/" + f.GetCombatState() + "/" + Num(f.GetActorValue("Confidence"))
        s += 1
    EndWhile
    r += "; IsFleeing " + Probe(f, PS_Fleeing, PE_Fleeing) + ", events " + (f as VrmAt21ActorLog).Take() + "\n"
    f.StopCombat()
    Gone(f)
    Gone(t)
    Reset()
    return r
EndFunction

String Function TestHolds()
    String r = FearCase(LowFighterLoggedBase, "a level 1 fighter") + FearCase(Level8LoggedBase, "a level 8 NPC")
    Actor c = SpawnWh(LowFighterLoggedBase, 0.0, 0.0)
    Actor ct = SpawnWh(InvulnerableBase, 200.0, 0.0)
    ct.SetDontMove(true)
    Utility.Wait(1.0)
    c.StartCombat(ct)
    Utility.Wait(3.0)
    (c as VrmAt21ActorLog).Take()
    r += "our Calm (9) on a level 1 fighter" + Near(c) + " (combat " + c.GetCombatState() + ", aggression " + Num(c.GetActorValue("Aggression")) + "): state/aggression every 2 s:"
    OurCalm.Cast(Player, c)
    Int s = 0
    While s < 4
        Utility.Wait(2.0)
        r += " " + c.GetCombatState() + "/" + Num(c.GetActorValue("Aggression"))
        s += 1
    EndWhile
    r += "; events " + (c as VrmAt21ActorLog).Take() + "\n"
    c.StopCombat()
    Gone(c)
    Gone(ct)
    Reset()
    Actor f = SpawnWh(LowLoggedBase, 0.0, 0.0)
    Actor n = SpawnWh(NakedBase, 200.0, 0.0)
    n.SetDontMove(true)
    Utility.Wait(1.0)
    (f as VrmAt21ActorLog).Take()
    r += "our Fury (6) on a level 1 unaggressive NPC" + Near(f) + ", another 200 off: state/target/aggression every 2 s:"
    OurFury.Cast(Player, f)
    s = 0
    While s < 4
        Utility.Wait(2.0)
        String who = "none"
        If f.GetCombatTarget() == n
            who = "other"
        ElseIf f.GetCombatTarget() == Player
            who = "player"
        EndIf
        r += " " + f.GetCombatState() + "/" + who + "/" + Num(f.GetActorValue("Aggression"))
        s += 1
    EndWhile
    r += "; events " + (f as VrmAt21ActorLog).Take() + "\n"
    f.StopCombat()
    n.StopCombat()
    Gone(f)
    Gone(n)
    Reset()
    return r
EndFunction

; disarm: what decides it (autotest24: casts within the level cap missed
; now and then, on NPCs mid-fight). A fresh NPC each case: alerted and
; drawn out of combat, fighting a free opponent and caught mid-swing
; (IsAttacking), or fighting and caught between swings; above the cap
; (level 15 at 12) with 30 as the control.
String Function Disarmed(Actor a, Spell s)
    Bool attacking = a.GetAnimationVariableBool("IsAttacking")
    String r = "drawn " + a.IsWeaponDrawn() + ", combat " + a.GetCombatState() + ", attacking " + attacking + ", carries " + a.GetItemCount(IronSword)
    s.Cast(Player, a)
    Utility.Wait(2.0)
    return r + "; after: holds " + Id(a.GetEquippedWeapon(false)) + ", carries " + a.GetItemCount(IronSword) + ", events " + (a as VrmAt21ActorLog).Take()
EndFunction

String Function DisarmIdle(ActorBase b, Spell s, String label)
    Actor a = SpawnWh(b, 0.0, 0.0)
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    Player.MoveTo(a, 0.0, -300.0, 0.0, false)
    Utility.Wait(1.0)
    a.SetAlert(true)
    a.DrawWeapon()
    Utility.Wait(2.5)
    (a as VrmAt21ActorLog).Take()
    String r = "  " + label + " (level " + a.GetLevel() + "), alerted, out of combat: " + Disarmed(a, s) + "\n"
    a.SetAlert(false)
    Gone(a)
    SweepWh(IronSword)
    Reset()
    return r
EndFunction

; A fighter fighting a free opponent; the cast is made once the fighter is
; (swinging) or isn't (between) attacking, up to 10 s after it engages.
String Function DisarmFighting(Bool swinging, String label)
    Actor a = SpawnWh(LowFighterLoggedBase, 0.0, 0.0)
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
        If a.GetAnimationVariableBool("IsAttacking") == swinging && a.GetDistance(t) < 200.0
            found = true
        Else
            Utility.Wait(0.05)
        EndIf
    EndWhile
    (a as VrmAt21ActorLog).Take()
    String r = "  " + label + " (engaged after " + Num(e) + " s, waited " + Num(Now() - start) + " s): " + Disarmed(a, OurDisarm) + "\n"
    a.StopCombat()
    t.StopCombat()
    Gone(a)
    Gone(t)
    SweepWh(IronSword)
    Reset()
    return r
EndFunction

String Function TestDisarmIdle()
    String r = "our Disarm (12) on a level 1 NPC with a sword:\n"
    r += DisarmIdle(LowLoggedBase, OurDisarm, "1")
    r += DisarmIdle(LowLoggedBase, OurDisarm, "2")
    r += DisarmIdle(LowLoggedBase, OurDisarm, "3")
    r += DisarmIdle(LowLoggedBase, OurDisarm, "4")
    return r
EndFunction

String Function TestDisarmSwinging()
    String r = "our Disarm (12) on a level 1 fighter mid-swing:\n"
    r += DisarmFighting(true, "1")
    r += DisarmFighting(true, "2")
    r += DisarmFighting(true, "3")
    r += DisarmFighting(true, "4")
    return r
EndFunction

String Function TestDisarmBetween()
    String r = "our Disarm (12) on a level 1 fighter between swings:\n"
    r += DisarmFighting(false, "1")
    r += DisarmFighting(false, "2")
    r += DisarmFighting(false, "3")
    r += DisarmFighting(false, "4")
    return r
EndFunction

String Function TestDisarmAboveCap()
    String r = "our Disarm on a level 15 NPC, alerted and drawn:\n"
    r += DisarmIdle(Level15FighterBase, OurDisarm, "12, 1")
    r += DisarmIdle(Level15FighterBase, OurDisarm, "12, 2")
    r += DisarmIdle(Level15FighterBase, OurDisarm, "12, 3")
    r += DisarmIdle(Level15FighterBase, OurDisarmStrong, "30, 1")
    r += DisarmIdle(Level15FighterBase, OurDisarmStrong, "30, 2")
    return r
EndFunction

; status conditions: what makes GetDisease true. A skeever (its attacks
; carry DiseaseAtaxia, the race's attack data) attacks the player, who
; doesn't fight back, for up to 120 s or until the player has Ataxia.
; The player's health is topped up; each drop counts as a hit.
String Function TestDisease()
    Player.RemoveSpell(Ataxia)
    Utility.Wait(1.0)
    String r = "the player: DiseaseResist " + Num(Player.GetActorValue("DiseaseResist")) + ", HasSpell(Ataxia) " + Player.HasSpell(Ataxia) + ", GetDisease " + Probe(Player, PS_Disease, PE_Disease) + "\n"
    Actor k = SpawnWh(SkeeverBase, 0.0, 300.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(0.5)
    k.StartCombat(Player)
    Float start = Now()
    Int hits = 0
    Float last = Health(Player)
    While !Player.HasSpell(Ataxia) && Now() - start < 120.0
        Utility.Wait(0.5)
        Float h = Health(Player)
        If h < last - 0.5
            hits += 1
        EndIf
        If h < Player.GetActorValueMax("Health") - 300.0
            Player.RestoreActorValue("Health", 2000.0)
            h = Health(Player)
        EndIf
        last = h
    EndWhile
    r += "a skeever (level " + k.GetLevel() + ") attacking: " + hits + " hits in " + Num(Now() - start) + " s, combat " + k.GetCombatState() + ", HasSpell(Ataxia) " + Player.HasSpell(Ataxia) + "\n"
    k.StopCombat()
    Gone(k)
    Player.StopCombatAlarm()
    Utility.Wait(1.0)
    r += "after: GetDisease " + Probe(Player, PS_Disease, PE_Disease)
    Player.RemoveSpell(Ataxia)
    Utility.Wait(1.0)
    r += "; Ataxia removed: HasSpell " + Player.HasSpell(Ataxia) + ", GetDisease " + Probe(Player, PS_Disease, PE_Disease) + "\n"
    Reset()
    return r
EndFunction

; npc-healing: the Vigilant free and fighting, hurt by a fighter's blows
; (autotest24 lowered its health by script against a held target), its
; health at 40% to start; then wards: a fire warlock attacks it.
String Function Watch(Actor h, Actor a, Float seconds)
    String r = ""
    Float start = Now()
    While Now() - start < seconds
        Utility.Wait(4.0)
        r += " " + Num(Now() - start) + ":" + Num(h.GetActorValuePercentage("Health")) + "/" + Num(h.GetActorValue("Magicka")) + "/" + h.GetCombatState()
    EndWhile
    return r
EndFunction

String Function HealerCase(ActorBase attacker, String label)
    Actor h = SpawnWh(VigilantLogBase, 0.0, 0.0)
    Actor a = SpawnWh(attacker, 300.0, 0.0)
    Tough(a)
    h.ForceActorValue("HealRate", 0.0)
    h.RestoreActorValue("Magicka", 1000.0)
    Player.MoveTo(WhCenter, -700.0, 0.0, 0.0, false)
    Utility.Wait(1.5)
    h.DamageActorValue("Health", h.GetActorValueMax("Health") * 0.6)
    (h as VrmAt21ActorLog).Take()
    Fight(a, h)
    String r = "a Vigilant (level " + h.GetLevel() + ", health " + Num(h.GetActorValueMax("Health")) + ", spells " + Id(h.GetEquippedSpell(1)) + " / " + Id(h.GetEquippedSpell(0)) + ") against " + label + "; health fraction/magicka/state every 4 s:"
    r += Watch(h, a, 40.0)
    r += "\n  attacker's health lost " + Num(5000.0 - Health(a)) + ", the Vigilant's events " + (h as VrmAt21ActorLog).Take() + "\n"
    h.StopCombat()
    a.StopCombat()
    Gone(h)
    Gone(a)
    Reset()
    return r
EndFunction

String Function TestHealerHurt()
    return HealerCase(LowFighterBase, "a level 1 fighter with a sword (5000 health)")
EndFunction

String Function TestWardsFree()
    return HealerCase(WarlockLogBase, "a fire warlock (5000 health)")
EndFunction

; magic: a lawful NPC, free, hit by a script cast of the player's fire
; spell (autotest21's was held: it searched).
String Function TestHostileSpellFree()
    Actor a = SpawnWh(LoggedBase, 0.0, 400.0)
    a.SetCrimeFaction(CrimeFaction)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(1.5)
    (a as VrmAt21ActorLog).Take()
    CrimeFaction.SetCrimeGold(0)
    WhFireSpell.Cast(Player, a)
    String r = "a fire spell cast by script from the player at a free lawful NPC 400 off; state/targets the player/distance at:"
    Float start = Now()
    Int i = 0
    While i < 5
        Utility.Wait(2.0)
        r += " " + Num(Now() - start) + "s " + a.GetCombatState() + "/" + (a.GetCombatTarget() == Player) + "/" + Num(a.GetDistance(Player))
        i += 1
    EndWhile
    r += "; drawn " + a.IsWeaponDrawn() + ", crime gold " + CrimeFaction.GetCrimeGold() + ", events " + (a as VrmAt21ActorLog).Take() + "\n"
    a.StopCombat()
    CrimeFaction.SetCrimeGold(0)
    Gone(a)
    Reset()
    return r
EndFunction

; detection: an unalerted NPC, free, told to DrawWeapon (autotest21's was
; held: it sheathed 0.62 s later). Twice once, then drawn again at 0.3 s.
String Function SheatheTime(Float again)
    Actor a = SpawnWh(LowLoggedBase, 0.0, 300.0)
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    Utility.Wait(1.5)
    String r = "alerted " + a.IsAlerted() + ", drawn " + a.IsWeaponDrawn()
    a.DrawWeapon()
    Float t0 = Now()
    Float seen = -1.0
    Bool redrawn = false
    While (a.IsWeaponDrawn() || Now() - t0 < 0.5) && Now() - t0 < 15.0
        If seen < 0.0 && a.IsWeaponDrawn()
            seen = Now() - t0
        EndIf
        If again > 0.0 && !redrawn && Now() - t0 >= again
            a.DrawWeapon()
            redrawn = true
            r += ", drawn again at " + Num(Now() - t0)
        EndIf
        Utility.Wait(0.05)
    EndWhile
    r += "; drawn from " + Num(seen) + ", sheathed at " + Num(Now() - t0) + " s, moved " + Num(a.GetDistance(WhCenter) - 300.0) + "\n"
    a.SetAlert(false)
    Gone(a)
    SweepWh(IronSword)
    return r
EndFunction

String Function TestSheatheFree()
    String r = "  once: " + SheatheTime(-1.0)
    r += "  once again: " + SheatheTime(-1.0)
    r += "  again at 0.3 s: " + SheatheTime(0.3)
    return r + "  again at 0.3 s, 2: " + SheatheTime(0.3)
EndFunction

; actor holds: whether a held NPC fights an enemy within reach that
; attacks it: restrained, SetDontMove, free. Read: its combat state, swings
; seen (IsAttacking), the health it took off the enemy, its move.
String Function HeldFight(Int hold, String label)
    Actor s = SpawnWh(LowFighterLoggedBase, 0.0, 0.0)
    Actor e = SpawnWh(LowFighterBase, 100.0, 0.0)
    Tough(s)
    Tough(e)
    If hold == 1
        s.SetRestrained(true)
    ElseIf hold == 2
        s.SetDontMove(true)
    EndIf
    Utility.Wait(1.5)
    Float x = s.GetPositionX()
    Float y = s.GetPositionY()
    Fight(s, e)
    Int swings = 0
    Bool was = false
    Float start = Now()
    While Now() - start < 12.0
        Bool atk = s.GetAnimationVariableBool("IsAttacking")
        If atk && !was
            swings += 1
        EndIf
        was = atk
        Utility.Wait(0.1)
    EndWhile
    Float dx = s.GetPositionX() - x
    Float dy = s.GetPositionY() - y
    String r = "  " + label + ": state " + s.GetCombatState() + ", drawn " + s.IsWeaponDrawn() + ", swings " + swings + ", enemy's health lost " + Num(5000.0 - Health(e)) + ", its own " + Num(5000.0 - Health(s)) + ", moved " + Num(Math.Sqrt(dx * dx + dy * dy)) + "\n"
    s.SetRestrained(false)
    s.SetDontMove(false)
    s.StopCombat()
    e.StopCombat()
    Gone(s)
    Gone(e)
    SweepWh(IronSword)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestRestrainedFights()
    Player.MoveTo(WhCenter, -600.0, 0.0, 0.0, false)
    String r = "a level 1 fighter and an enemy 100 off fighting each other, 12 s:\n"
    r += HeldFight(1, "restrained")
    r += HeldFight(2, "SetDontMove")
    return r + HeldFight(0, "free")
EndFunction

; crime: Attack on Sight, a longer watch (autotest24: at 999 and 1000 the
; IsGuard guard walked up but didn't attack in 6 s). Bounty 1000, read
; every 5 s for 40 s.
String Function Views(Actor a)
    return "hostile " + a.IsHostileToActor(Player) + ", detects " + Player.IsDetectedBy(a) + ", state " + a.GetCombatState() + ", targets the player " + (a.GetCombatTarget() == Player) + ", drawn " + a.IsWeaponDrawn() + ", distance " + Num(a.GetDistance(Player))
EndFunction

String Function TestAttackOnSightLong()
    Actor g = SpawnWh(AosIsGuardBase, 500.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(3.0)
    String r = "a guard (IsGuard " + g.IsGuard() + ") of our faction with Arrest and Attack on Sight, 500 off: bounty 0, " + Views(g) + "\n"
    AosFaction.SetCrimeGold(1000)
    Float start = Now()
    While Now() - start < 40.0
        Utility.Wait(5.0)
        r += "  bounty 1000 +" + Num(Now() - start) + " s: " + Views(g) + ", player's health " + Num(Health(Player)) + "\n"
    EndWhile
    AosFaction.SetCrimeGold(0)
    g.StopCombat()
    Gone(g)
    Reset()
    return r
EndFunction

String Function TestLeaveWarehouse()
    Player.ModActorValue("Health", -1000.0)
    GoCell()
    return "back in the test cell\n"
EndFunction
