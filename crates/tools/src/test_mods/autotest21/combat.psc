; Combat in WarehouseFurniture (navmeshed, empty): assistance, friendly hits,
; fleeing, factions turning hostile, casters, healers, wards, staffs.
; The player gets 1000 extra health for these and loses it at the end.

String Function TestEnterWarehouse()
    GoWarehouse()
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 1000.0)
    return "in the warehouse: interior " + Player.IsInInterior() + ", WhCenter distance " + Num(Player.GetDistance(WhCenter)) + "\n"
EndFunction

; Red attacks a Teal victim; Teal helpers (allies) and a helper bound only
; by relationship rank, at distances: who joins.
String Function TestAssistance()
    Actor victim = SpawnWh(TealHelperBase, 0.0, 0.0)
    Actor attacker = SpawnWh(RedFighterBase, 150.0, 0.0)
    Actor near = SpawnWh(TealHelperBase, 0.0, 500.0)
    Actor far = SpawnWh(TealHelperBase, 0.0, -1200.0)
    Actor friend = SpawnWh(HelperBase, -500.0, 0.0)
    friend.SetRelationshipRank(victim, 3)
    victim.SetRelationshipRank(friend, 3)
    Utility.Wait(2.0)
    attacker.StartCombat(victim)
    String r = "Red attacks a Teal NPC (helpers assist allies):"
    Int s = 0
    While s < 5
        Utility.Wait(2.0)
        r += "\n  " + ((s + 1) * 2) + " s: victim " + victim.GetCombatState() + ", Teal at 500 " + near.GetCombatState() + ", Teal at 1200 " + far.GetCombatState() + ", a confidant (RELA 3, no faction) " + friend.GetCombatState()
        s += 1
    EndWhile
    r += "\n  their targets: near " + Id(near.GetCombatTarget()) + ", far " + Id(far.GetCombatTarget()) + ", confidant " + Id(friend.GetCombatTarget()) + " (attacker " + Id(attacker) + ")\n"
    attacker.Kill(None)
    victim.StopCombat()
    near.StopCombat()
    far.StopCombat()
    friend.StopCombat()
    Gone(attacker)
    Gone(victim)
    Gone(near)
    Gone(far)
    Gone(friend)
    return r
EndFunction

; Fire spells from one Teal member at another until it fights back.
String Function TestFriendlyHits()
    Actor a = SpawnWh(TealHelperBase, 0.0, 0.0)
    Actor b = SpawnWh(TealHelperBase, 300.0, 0.0)
    b.ForceActorValue("HealRate", 0.0)
    Utility.Wait(2.0)
    String r = "a Teal member's fire spells at another:"
    Int i = 0
    While i < 8 && b.GetCombatState() == 0
        WhFireSpell.Cast(a, b)
        Utility.Wait(1.5)
        i += 1
        r += " " + i + ":" + b.GetCombatState()
    EndWhile
    r += "; health " + Num(b.GetActorValue("Health")) + "\n"
    a.StopCombat()
    b.StopCombat()
    Gone(a)
    Gone(b)
    return r
EndFunction

; Who flees and when: an Average and a Cowardly NPC set on a strong brute,
; distances and states sampled.
String Function TestFlee()
    String r = ""
    Int k = 0
    While k < 3
        Actor f
        String label
        If k == 0
            f = SpawnWh(AverageBase, 0.0, 0.0)
            label = "an Average NPC, unharmed"
        ElseIf k == 1
            f = SpawnWh(AverageBase, 0.0, 0.0)
            label = "an Average NPC at 40% health"
        Else
            f = SpawnWh(CowardBase, 0.0, 0.0)
            label = "a Cowardly NPC"
        EndIf
        Actor brute = SpawnWh(BruteBase, 250.0, 0.0)
        Utility.Wait(1.5)
        If k == 1
            f.DamageActorValue("Health", f.GetActorValue("Health") * 0.6)
        EndIf
        f.StartCombat(brute)
        brute.StartCombat(f)
        r += label + " set on a level 50 brute; distance by second:"
        Int s = 0
        While s < 12
            Utility.Wait(1.0)
            r += " " + Num(f.GetDistance(brute)) + "/" + f.GetCombatState()
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

; factions.md: the player-enemy flag, attack on sight, aggression 0.
String Function TestPlayerEnemy()
    Actor brave = SpawnWh(RedFighterBase, 400.0, 0.0)
    Actor meek = SpawnWh(RedBase, -400.0, 0.0)
    Actor ally = SpawnWh(GreenFighterBase, 0.0, 400.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(2.0)
    RedFaction.SetPlayerEnemy(true)
    Utility.Wait(4.0)
    String r = "SetPlayerEnemy on Red: an aggressive member's state " + brave.GetCombatState() + " (target player " + (brave.GetCombatTarget() == Player) + "), an unaggressive member's " + meek.GetCombatState() + ", distance " + Num(meek.GetDistance(Player))
    RedFaction.SetPlayerEnemy(false)
    brave.StopCombat()
    meek.StopCombat()
    Utility.Wait(2.0)
    r += "; cleared and StopCombat: " + brave.GetCombatState() + "\n"
    Player.SetAttackActorOnSight(true)
    Utility.Wait(4.0)
    r += "player SetAttackActorOnSight: aggressive neutral " + brave.GetCombatState() + ", unaggressive " + meek.GetCombatState() + ", Green aggressive " + ally.GetCombatState() + "\n"
    Player.SetAttackActorOnSight(false)
    brave.StopCombat()
    ally.StopCombat()
    meek.StopCombat()
    Utility.Wait(1.0)
    brave.SetAttackActorOnSight(true)
    Utility.Wait(4.0)
    r += "an NPC marked attack-on-sight: Green aggressive attacks it " + ally.GetCombatState() + " (target " + (ally.GetCombatTarget() == brave) + ")\n"
    brave.StopCombat()
    ally.StopCombat()
    Gone(brave)
    Gone(meek)
    Gone(ally)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; The player in a fight: a teammate joins; GetCombatState of the player.
String Function TestTeammate()
    Actor mate = SpawnWh(HelperBase, -300.0, 0.0)
    mate.SetPlayerTeammate(true, true)
    Actor foe = SpawnWh(RedFighterBase, 400.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(2.0)
    foe.StartCombat(Player)
    Utility.Wait(1.0)
    String r = "an NPC attacks the player: player's GetCombatState " + Player.GetCombatState() + ", teammate's"
    Int s = 0
    While s < 4
        Utility.Wait(1.0)
        r += " " + mate.GetCombatState()
        s += 1
    EndWhile
    r += " (target the foe " + (mate.GetCombatTarget() == foe) + ")"
    foe.Kill(mate)
    Utility.Wait(1.0)
    r += "; the teammate kills it: Murders " + Game.QueryStat("Murders") + ", killer " + Id(foe.GetKiller()) + "\n"
    mate.SetPlayerTeammate(false, false)
    mate.StopCombat()
    Gone(mate)
    Gone(foe)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; Casters: which hand, how often, from how far.
String Function TestCasters()
    Actor c = SpawnWh(MageBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 900.0, 0.0)
    t.SetDontMove(true)
    Utility.Wait(2.0)
    (c as VrmAt21ActorLog).Take()
    c.StartCombat(t)
    String r = "a mage (Firebolt, magic combat style) on a held target 900 off:"
    Int s = 0
    While s < 10
        Utility.Wait(1.0)
        r += " " + Num(c.GetDistance(t))
        s += 1
    EndWhile
    r += "\n  right " + Id(c.GetEquippedSpell(1)) + ", left " + Id(c.GetEquippedSpell(0)) + " (Firebolt " + Id(FireboltSpell) + "); events " + (c as VrmAt21ActorLog).Take() + "\n"
    c.StopCombat()
    Gone(c)
    Gone(t)
    return r
EndFunction

; npc-healing: a fighter that knows Healing, hurt.
String Function TestHealer()
    Actor h = SpawnWh(HealerBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
    t.SetDontMove(true)
    h.ForceActorValue("HealRate", 0.0)
    Utility.Wait(2.0)
    (h as VrmAt21ActorLog).Take()
    h.StartCombat(t)
    String r = "a fighter knowing Healing, by health fraction:"
    Float[] at = new Float[3]
    at[0] = 0.7
    at[1] = 0.45
    at[2] = 0.2
    Int i = 0
    While i < 3
        h.RestoreActorValue("Health", 1000.0)
        h.DamageActorValue("Health", h.GetActorValueMax("Health") * (1.0 - at[i]))
        Float before = h.GetActorValuePercentage("Health")
        Utility.Wait(6.0)
        r += " at " + Num(before) + " -> " + Num(h.GetActorValuePercentage("Health")) + " (" + (h as VrmAt21ActorLog).Take() + ")"
        i += 1
    EndWhile
    r += "\n"
    h.StopCombat()
    Gone(h)
    Gone(t)
    return r
EndFunction

; wards: a Vigilant of Stendarr (Lesser Ward) set on a fire warlock.
String Function TestWards()
    Actor v = SpawnWh(VigilantBase, 0.0, 0.0)
    Actor w = SpawnWh(WarlockBase, 700.0, 0.0)
    v.SetDontMove(true)
    w.SetDontMove(true)
    v.ForceActorValue("HealRate", 0.0)
    Utility.Wait(2.0)
    (v as VrmAt21ActorLog).Take()
    v.StartCombat(w)
    w.StartCombat(v)
    String r = "Vigilant warded? WardPower / warlock magicka every 0.5 s:"
    Int s = 0
    While s < 30
        Utility.Wait(0.5)
        r += " " + Num(v.GetActorValue("WardPower")) + "/" + Num(w.GetActorValue("Magicka"))
        s += 1
    EndWhile
    r += "\n  Vigilant's events " + (v as VrmAt21ActorLog).Take() + "\n"
    v.StopCombat()
    w.StopCombat()
    Gone(v)
    Gone(w)
    return r
EndFunction

; staffs: an NPC with Sparks staff fighting: its charge.
String Function TestNpcStaff()
    Actor s = SpawnWh(StaffBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 500.0, 0.0)
    t.SetDontMove(true)
    Utility.Wait(1.0)
    s.AddItem(SparksStaff, 1, true)
    s.EquipItem(SparksStaff, false, true)
    Utility.Wait(1.5)
    String r = "a staff of sparks: equipped " + s.IsEquipped(SparksStaff) + ", RightItemCharge " + Num(s.GetActorValue("RightItemCharge")) + ", LeftItemCharge " + Num(s.GetActorValue("LeftItemCharge"))
    s.StartCombat(t)
    Int k = 0
    While k < 8
        Utility.Wait(1.5)
        r += " " + Num(s.GetActorValue("RightItemCharge"))
        k += 1
    EndWhile
    r += "; the target's health " + Num(t.GetActorValue("Health")) + "\n"
    s.StopCombat()
    Gone(s)
    Gone(t)
    return r
EndFunction
