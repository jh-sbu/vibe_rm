; Fights in WarehouseFurniture: autotest21's that didn't happen. Casters are
; copies of vanilla ones with our logger (the warlock cast in autotest21,
; our own level 10 casters never did); fights get health enough to last.
; The player gets 1000 extra health here and loses it on leaving.

String Function TestEnterWarehouse()
    GoWarehouse()
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 1000.0)
    return "in the warehouse: interior " + Player.IsInInterior() + ", WhCenter distance " + Num(Player.GetDistance(WhCenter)) + "\n"
EndFunction

; How a caster fights: distance by second, the hands' spells, its casts and
; magicka.
String Function TestCasters()
    Actor c = SpawnWh(WarlockLogBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 900.0, 0.0)
    t.SetDontMove(true)
    Utility.Wait(2.0)
    (c as VrmAt21ActorLog).Take()
    Float m0 = c.GetActorValue("Magicka")
    c.StartCombat(t)
    String r = "a fire warlock (level " + c.GetLevel() + ", Destruction " + c.GetActorValue("Destruction") + ", magicka " + Num(m0) + ") on a held target 900 off; distance / magicka by second:"
    Int s = 0
    While s < 12
        Utility.Wait(1.0)
        r += " " + Num(c.GetDistance(t)) + "/" + Num(c.GetActorValue("Magicka"))
        s += 1
    EndWhile
    r += "\n  right " + Id(c.GetEquippedSpell(1)) + ", left " + Id(c.GetEquippedSpell(0)) + " (Firebolt " + Id(FireboltSpell) + ", Flames " + Id(FlamesSpell) + "); events " + (c as VrmAt21ActorLog).Take() + "\n"
    c.StopCombat()
    Gone(c)
    Gone(t)
    return r
EndFunction

; magic-menu: Destruction cost reduction on an NPC (no perk): magicka per
; cast at DestructionMod 0, 50 and -50.
String Function MagickaPerCast(Float mod)
    Actor c = SpawnWh(WarlockLogBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 600.0, 0.0)
    t.SetDontMove(true)
    c.ForceActorValue("MagickaRate", 0.0)
    c.ForceActorValue("DestructionMod", mod)
    Utility.Wait(1.0)
    (c as VrmAt21ActorLog).Take()
    Float m = c.GetActorValue("Magicka")
    c.StartCombat(t)
    String r = "DestructionMod " + mod + ": magicka by second from " + Num(m) + ":"
    Int s = 0
    While s < 8
        Utility.Wait(1.0)
        r += " " + Num(c.GetActorValue("Magicka"))
        s += 1
    EndWhile
    r += " (" + (c as VrmAt21ActorLog).Take() + ")"
    c.StopCombat()
    Gone(c)
    Gone(t)
    return r
EndFunction

String Function TestNpcCostReduction()
    return MagickaPerCast(0.0) + "\n" + MagickaPerCast(50.0) + "\n" + MagickaPerCast(-50.0) + "\n"
EndFunction

; npc-healing: a Vigilant of Stendarr (its own HealingLeftHand) hurt in a
; fight, at 70%, 45% and 20% of its health.
String Function TestHealer()
    Actor h = SpawnWh(VigilantLogBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
    t.SetDontMove(true)
    h.ForceActorValue("HealRate", 0.0)
    Utility.Wait(2.0)
    (h as VrmAt21ActorLog).Take()
    h.StartCombat(t)
    Utility.Wait(2.0)
    String r = "a Vigilant (level " + h.GetLevel() + ", Restoration " + h.GetActorValue("Restoration") + ") fighting, by health fraction:"
    Float[] at = new Float[3]
    at[0] = 0.7
    at[1] = 0.45
    at[2] = 0.2
    Int i = 0
    While i < 3
        h.RestoreActorValue("Health", 10000.0)
        h.DamageActorValue("Health", h.GetActorValueMax("Health") * (1.0 - at[i]))
        Float before = h.GetActorValuePercentage("Health")
        Float m = h.GetActorValue("Magicka")
        Utility.Wait(8.0)
        r += "\n  at " + Num(before) + " -> " + Num(h.GetActorValuePercentage("Health")) + ", magicka " + Num(m) + " -> " + Num(h.GetActorValue("Magicka")) + " (" + (h as VrmAt21ActorLog).Take() + ")"
        i += 1
    EndWhile
    h.StopCombat()
    Gone(h)
    Gone(t)
    return r + "\n"
EndFunction

; wards: a Vigilant (Lesser Ward) set on a fire warlock: its health,
; WardPower and what it hears.
String Function TestWards()
    Actor v = SpawnWh(VigilantLogBase, 0.0, 0.0)
    Actor w = SpawnWh(WarlockLogBase, 700.0, 0.0)
    v.SetDontMove(true)
    w.SetDontMove(true)
    v.ForceActorValue("HealRate", 0.0)
    v.ForceActorValue("Health", 2000.0)
    Utility.Wait(2.0)
    (v as VrmAt21ActorLog).Take()
    (w as VrmAt21ActorLog).Take()
    v.StartCombat(w)
    w.StartCombat(v)
    String r = "a Vigilant held 700 from a held fire warlock; its health / WardPower every second:"
    Int s = 0
    While s < 15
        Utility.Wait(1.0)
        r += " " + Num(v.GetActorValue("Health")) + "/" + Num(v.GetActorValue("WardPower"))
        s += 1
    EndWhile
    r += "\n  the Vigilant's events " + (v as VrmAt21ActorLog).Take() + "\n  the warlock's " + (w as VrmAt21ActorLog).Take() + "\n"
    v.StopCombat()
    w.StopCombat()
    Gone(v)
    Gone(w)
    return r
EndFunction

; staffs: a warlock copy without spells, given a staff of sparks.
String Function TestNpcStaff()
    Actor s = SpawnWh(StaffWarlockBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 500.0, 0.0)
    t.SetDontMove(true)
    t.ForceActorValue("Health", 2000.0)
    Utility.Wait(1.0)
    s.AddItem(SparksStaff, 1, true)
    s.EquipItem(SparksStaff, false, true)
    Utility.Wait(1.5)
    (s as VrmAt21ActorLog).Take()
    String r = "a spell-less warlock with a staff of sparks: equipped " + s.IsEquipped(SparksStaff) + ", right " + Id(s.GetEquippedWeapon(false)) + ", left " + Id(s.GetEquippedWeapon(true)) + "; charge right/left by 1.5 s:"
    s.StartCombat(t)
    Int k = 0
    While k < 8
        Utility.Wait(1.5)
        r += " " + Num(s.GetActorValue("RightItemCharge")) + "/" + Num(s.GetActorValue("LeftItemCharge"))
        k += 1
    EndWhile
    r += "; the target's health " + Num(t.GetActorValue("Health")) + ", events " + (s as VrmAt21ActorLog).Take() + "\n"
    s.StopCombat()
    Gone(s)
    Gone(t)
    return r
EndFunction

; factions: a fight between enemies whose factions are made allies.
String Function TestFightAfterAlly()
    Actor a = SpawnWh(RedFighterBase, 0.0, 0.0)
    Actor b = SpawnWh(GreenFighterBase, 300.0, 0.0)
    a.ForceActorValue("Health", 5000.0)
    b.ForceActorValue("Health", 5000.0)
    Utility.Wait(1.0)
    a.StartCombat(b)
    b.StartCombat(a)
    Utility.Wait(3.0)
    String r = "Red and Green fighting: states " + a.GetCombatState() + "/" + b.GetCombatState()
    RedFaction.SetAlly(GreenFaction, true, true)
    Int s = 0
    While s < 8
        Utility.Wait(1.0)
        r += "; allied +" + (s + 1) + " s: " + a.GetCombatState() + "/" + b.GetCombatState()
        s += 1
    EndWhile
    RedFaction.SetEnemy(GreenFaction, false, false)
    a.StopCombat()
    b.StopCombat()
    Gone(a)
    Gone(b)
    return r + "\n"
EndFunction

; combat-fleeing: Average NPCs (300 health) set on a level 20 fighter:
; distance, combat state and health fraction by second.
String Function TestFlee()
    String r = ""
    Int k = 0
    While k < 2
        Actor f = SpawnWh(AverageBase, 0.0, 0.0)
        Actor brute = SpawnWh(Level20FighterBase, 250.0, 0.0)
        f.SetActorValue("Health", 300.0)
        Utility.Wait(1.5)
        String label = "an Average NPC, unharmed"
        If k == 1
            f.DamageActorValue("Health", f.GetActorValue("Health") * 0.6)
            label = "an Average NPC at 40% health"
        EndIf
        f.StartCombat(brute)
        brute.StartCombat(f)
        r += label + " set on a level 20 fighter; distance/state/health by second:"
        Int s = 0
        While s < 12
            Utility.Wait(1.0)
            r += " " + Num(f.GetDistance(brute)) + "/" + f.GetCombatState() + "/" + Num(f.GetActorValuePercentage("Health"))
            s += 1
        EndWhile
        r += "\n"
        brute.Kill(None)
        f.StopCombat()
        Gone(brute)
        Gone(f)
        Utility.Wait(1.0)
        k += 1
    EndWhile
    return r
EndFunction

; What an NPC's view of the player is: hostile, detecting, combat state and
; target.
String Function Views(Actor a)
    return "hostile " + a.IsHostileToActor(Player) + ", detects " + Player.IsDetectedBy(a) + ", state " + a.GetCombatState() + ", targets the player " + (a.GetCombatTarget() == Player)
EndFunction

; factions: the player-enemy flag and attack on sight, with what each NPC
; makes of the player.
String Function TestPlayerEnemy()
    Actor brave = SpawnWh(RedFighterBase, 400.0, 0.0)
    Actor meek = SpawnWh(RedBase, -400.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(3.0)
    String r = "before: aggressive Red " + Views(brave) + "\n"
    RedFaction.SetPlayerEnemy(true)
    Int s = 0
    While s < 2
        Utility.Wait(4.0)
        r += "SetPlayerEnemy +" + ((s + 1) * 4) + " s: aggressive " + Views(brave) + "; unaggressive " + Views(meek) + ", distance " + Num(meek.GetDistance(Player)) + "\n"
        s += 1
    EndWhile
    RedFaction.SetPlayerEnemy(false)
    brave.StopCombat()
    meek.StopCombat()
    Utility.Wait(2.0)
    Player.SetAttackActorOnSight(true)
    Utility.Wait(6.0)
    r += "player SetAttackActorOnSight +6 s: aggressive " + Views(brave) + "; unaggressive " + Views(meek) + "\n"
    Player.SetAttackActorOnSight(false)
    brave.StopCombat()
    meek.StopCombat()
    Gone(brave)
    Gone(meek)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; crime: a guard of a faction the player owes 1000 to.
String Function TestAttackOnSight()
    Actor g = SpawnWh(CrimeGuardBase, 500.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(3.0)
    String r = "a guard of the crime faction 500 off, bounty 0: " + Views(g) + "\n"
    CrimeFaction.SetCrimeGold(1000)
    Utility.Wait(6.0)
    r += "bounty 1000 +6 s: " + Views(g) + "\n"
    CrimeFaction.SetCrimeGold(0)
    CrimeFaction.SetCrimeGoldViolent(1000)
    Utility.Wait(6.0)
    r += "violent 1000 only +6 s: " + Views(g) + "\n"
    CrimeFaction.SetCrimeGoldViolent(0)
    g.StopCombat()
    Player.StopCombatAlarm()
    Gone(g)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; combat-state: IsInCombat's integer parameter, during a fight.
String Function TestInCombatParam()
    Actor a = SpawnWh(RedFighterBase, 0.0, 0.0)
    Actor b = SpawnWh(GreenFighterBase, 300.0, 0.0)
    a.ForceActorValue("Health", 5000.0)
    b.ForceActorValue("Health", 5000.0)
    Utility.Wait(1.0)
    a.StartCombat(b)
    Utility.Wait(2.0)
    String r = "fighting (state " + a.GetCombatState() + "): IsInCombat(0) " + Probe(a, PS_InCombat0, PE_InCombat0) + ", (1) " + Probe(a, PS_InCombat1, PE_InCombat1) + ", (2) " + Probe(a, PS_InCombat2, PE_InCombat2) + "\n"
    a.StopCombat()
    b.StopCombat()
    Gone(b)
    Utility.Wait(1.0)
    r += "after StopCombat, the foe gone (state " + a.GetCombatState() + "): (0) " + Probe(a, PS_InCombat0, PE_InCombat0) + ", (1) " + Probe(a, PS_InCombat1, PE_InCombat1) + "\n"
    Gone(a)
    return r
EndFunction

; magic: calm, fury and fear read by what the target does (autotest21 read
; the actor values, which these archetypes don't move).
; Fear (magnitude 9) on a level 8 NPC fighting a held target, at magic
; resistance 0, 50 and 90: does it run? Calm (9) on a level 1 fighter: does
; it stop? Fury (6) on a level 1 unaggressive NPC beside another: does it
; attack?
String Function TestHolds()
    String r = ""
    Float[] res = new Float[3]
    res[0] = 0.0
    res[1] = 50.0
    res[2] = 90.0
    Int i = 0
    While i < 3
        Actor b = SpawnWh(Level8Base, 0.0, 0.0)
        Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
        t.SetDontMove(true)
        b.ForceActorValue("MagicResist", res[i])
        b.AddItem(IronSword, 1, true)
        b.EquipItem(IronSword, false, true)
        Utility.Wait(1.0)
        b.StartCombat(t)
        Utility.Wait(3.0)
        Float d0 = b.GetDistance(t)
        FearSpell.Cast(Player, b)
        Utility.Wait(4.0)
        r += "Fear on a level 8 NPC, magic resistance " + res[i] + ": distance " + Num(d0) + " -> " + Num(b.GetDistance(t)) + ", combat " + b.GetCombatState() + "\n"
        b.StopCombat()
        Gone(b)
        Gone(t)
        i += 1
    EndWhile
    Actor c = SpawnWh(LowFighterBase, 0.0, 0.0)
    Actor ct = SpawnWh(InvulnerableBase, 200.0, 0.0)
    ct.SetDontMove(true)
    Utility.Wait(1.0)
    c.StartCombat(ct)
    Utility.Wait(3.0)
    r += "Calm on a level 1 fighter (combat " + c.GetCombatState() + "):"
    CalmSpell.Cast(Player, c)
    Int s = 0
    While s < 4
        Utility.Wait(1.0)
        r += " " + c.GetCombatState()
        s += 1
    EndWhile
    c.StopCombat()
    Gone(c)
    Gone(ct)
    Actor f = SpawnWh(LowBase, 0.0, 0.0)
    Actor n = SpawnWh(NakedBase, 200.0, 0.0)
    n.SetDontMove(true)
    Utility.Wait(1.0)
    FurySpell.Cast(Player, f)
    r += "\nFury on a level 1 unaggressive NPC, another 200 off: combat state, target"
    s = 0
    While s < 5
        Utility.Wait(1.0)
        String who = "none"
        If f.GetCombatTarget() == n
            who = "the other"
        ElseIf f.GetCombatTarget() == Player
            who = "the player"
        EndIf
        r += " " + f.GetCombatState() + "/" + who
        s += 1
    EndWhile
    f.StopCombat()
    n.StopCombat()
    Gone(f)
    Gone(n)
    Player.RestoreActorValue("Health", 2000.0)
    return r + "\n"
EndFunction

String Function TestLeaveWarehouse()
    Player.ModActorValue("Health", -1000.0)
    GoCell()
    return "back in the test cell\n"
EndFunction
