; Story Manager events (each type's quest node of ours reports through
; AddStory) and quest aliases, in the test cell.

String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + "; "
EndFunction

String Function TakeStory()
    Utility.Wait(1.5)
    String s = StoryLog
    StoryLog = ""
    If s == ""
        return "(none)"
    EndIf
    return s
EndFunction

String Function TestStoryItems()
    String r = "(player " + Id(Player) + ")\n"
    Player.AddItem(IngotIron, 1, false)
    r += "AddItem(ingot) to the player: " + TakeStory() + "\n"
    ObjectReference dropped = Player.DropObject(IngotIron, 1)
    r += "DropObject(ingot, ref " + Id(dropped) + "): " + TakeStory() + "\n"
    dropped.Activate(Player, false)
    Utility.Wait(1.0)
    r += "picked up again by Activate(player): " + TakeStory() + "\n"
    ObjectReference c = Put(EmptyChest, 200.0, 0.0, 0.0)
    Player.RemoveItem(IngotIron, 1, false, c)
    r += "RemoveItem into a chest (" + Id(c) + "): " + TakeStory() + "\n"
    Actor n = Spawn(NakedBase, 0.0, 300.0)
    n.SetDontMove(true)
    Player.AddItem(IngotIron, 1, true)
    TakeStory()
    Player.RemoveItem(IngotIron, 1, false, n)
    r += "RemoveItem to an NPC (" + Id(n) + "): " + TakeStory() + "\n"
    ObjectReference owned = Put(IngotIron, 0.0, 150.0, 20.0)
    owned.SetActorOwner(NakedBase)
    Utility.Wait(0.5)
    owned.Activate(Player, false)
    r += "an ingot owned by an NPC base picked up: " + TakeStory() + "\n"
    c.RemoveItem(IngotIron, 1, false, Player)
    r += "taken from the chest by RemoveItem: " + TakeStory() + "\n"
    Player.RemoveItem(IngotIron, Player.GetItemCount(IngotIron), true, None)
    Gone(c)
    Gone(n)
    return r
EndFunction

String Function TestStoryCombat()
    String r = ""
    Actor n = Spawn(NakedBase, 0.0, 300.0)
    n.SetDontMove(true)
    n.ForceActorValue("Health", 500.0)
    Utility.Wait(1.0)
    TakeStory()
    WhFireSpell.Cast(Player, n)
    Utility.Wait(1.0)
    r += "a fire spell from the player at an NPC (" + Id(n) + "): " + TakeStory() + "\n"
    WhFireSpell.Cast(Player, n)
    r += "a second: " + TakeStory() + "\n"
    WhFireSpell.Cast(n, Alpha)
    r += "an NPC's spell at another (Alpha " + Id(Alpha) + "): " + TakeStory() + "\n"
    n.Kill(Player)
    r += "Kill(player): " + TakeStory() + "\n"
    Actor m = Spawn(NakedBase, 300.0, 300.0)
    m.SetCrimeFaction(CrimeFaction)
    Utility.Wait(0.5)
    TakeStory()
    m.Kill(Alpha)
    r += "a lawful NPC killed by Alpha (crime faction): " + TakeStory() + "\n"
    Game.IncrementSkill("Sneak")
    r += "IncrementSkill(Sneak): " + TakeStory() + "\n"
    Alpha.SetRelationshipRank(Beta, 2)
    r += "SetRelationshipRank(Alpha, Beta, 2): " + TakeStory() + "\n"
    Alpha.SetRelationshipRank(Beta, 0)
    TakeStory()
    Gone(n)
    Gone(m)
    return r
EndFunction

; alias-fills: what happens to an alias's created references.
String Function RefState(ObjectReference o)
    If o == None
        return "none"
    EndIf
    return Id(o) + " disabled " + o.IsDisabled() + " deleted " + o.IsDeleted() + " 3D " + o.Is3DLoaded()
EndFunction

String Function TestCreatedRefs()
    CreateQuest.Start()
    Utility.Wait(1.0)
    ReferenceAlias made = CreateQuest.GetAlias(1) as ReferenceAlias
    ReferenceAlias item = CreateQuest.GetAlias(2) as ReferenceAlias
    ObjectReference a1 = made.GetReference()
    ObjectReference i1 = item.GetReference()
    String r = "created: actor " + RefState(a1) + "; item " + RefState(i1) + "\n"
    made.Clear()
    item.Clear()
    Utility.Wait(1.0)
    r += "aliases cleared: actor " + RefState(a1) + "; item " + RefState(i1) + "\n"
    CreateQuest.Stop()
    Utility.Wait(2.0)
    r += "quest stopped: actor " + RefState(a1) + "; item " + RefState(i1) + "\n"
    CreateQuest.Start()
    Utility.Wait(1.0)
    ObjectReference a2 = made.GetReference()
    ObjectReference i2 = item.GetReference()
    CreateQuest.CompleteQuest()
    Utility.Wait(1.0)
    r += "started again, CompleteQuest: actor " + RefState(a2) + "; item " + RefState(i2) + ", running " + CreateQuest.IsRunning() + "\n"
    CreateQuest.Stop()
    Utility.Wait(2.0)
    r += "then stopped: actor " + RefState(a2) + "; item " + RefState(i2) + "\n"
    Gone(a1)
    Gone(a2)
    Gone(i1)
    Gone(i2)
    return r
EndFunction

String Function TestAliasMatch()
    Actor near = Spawn(MatchBase, 0.0, 200.0)
    Utility.Wait(1.0)
    String r = "matching NPCs: a plugin one in this cell " + Id(MatchFar) + ", one in the warehouse " + Id(MatchWh) + ", one placed by script near the player " + Id(near) + "\n"
    Int i = 0
    While i < 3
        Quest q = MatchQuests.GetAt(i) as Quest
        q.Start()
        Utility.Wait(1.0)
        ReferenceAlias al = q.GetAlias(0) as ReferenceAlias
        If i == 0
            r += "  no flags: "
        ElseIf i == 1
            r += "  in loaded area: "
        Else
            r += "  in loaded area, closest: "
        EndIf
        r += Id(al.GetReference()) + "\n"
        q.Stop()
        i += 1
    EndWhile
    Gone(near)
    i = 0
    While i < 2
        Quest q2 = LevelQuests.GetAt(i) as Quest
        q2.Start()
        Utility.Wait(1.0)
        Actor made = (q2.GetAlias(1) as ReferenceAlias).GetActorReference()
        r += "a leveled bandit created at level " + (i * 3) + " (easy/very hard): level " + made.GetLevel() + " base " + Id(made.GetLeveledActorBase()) + "\n"
        q2.Stop()
        Gone(made)
        i += 1
    EndWhile
    return r
EndFunction
