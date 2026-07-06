//! The computed report sections: tables and figures derived from declared
//! data and the newest snapshot. Authored prose arrives pre-rendered from
//! the editorial fragments.

use std::fmt::Write as _;

use rollcall_inventory::{CapexFile, Device, DevicesFile, SiteFile, ThermalFile, WorkloadsFile};
use rollcall_status::{ProbeState, Snapshot};

use crate::power::{PowerModel, usd_per_month};
use crate::tex::{esc, fmt_int_sep, fmt_sep, state_badge};

/// Live-status table, prefaced by the rendered `status-note` fragment.
pub fn status_section(snapshot: &Snapshot, note: &str) -> String {
    let rows = snapshot
        .results
        .iter()
        .map(|r| {
            format!(
                "{} & {} & {} \\\\",
                esc(&r.desc),
                state_badge(r.state),
                esc(&r.detail)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\n\\section{{Live Status Snapshot}}\n{note}\n\\begin{{tabularx}}{{\\textwidth}}{{X l l}}\n\\toprule\n\\textbf{{Target}} & \\textbf{{State}} & \\textbf{{Detail}} \\\\\n\\midrule\n{rows}\n\\bottomrule\n\\end{{tabularx}}\n"
    )
}

/// Per-device inventory subsections.
pub fn devices_section(devices: &DevicesFile) -> String {
    let blocks: Vec<String> = devices.device.iter().map(device_block).collect();
    format!("\n\\section{{Device Inventory}}\n{}", blocks.join("\n"))
}

fn device_block(dev: &Device) -> String {
    let mut lines = vec![format!("\\textbf{{Role:}} {}\\\\", esc(&dev.role))];
    let mut nets = Vec::new();
    if let Some(lan) = &dev.ip_lan {
        nets.push(format!("\\textbf{{LAN:}} \\texttt{{{lan}}}"));
    }
    if let Some(fabric) = &dev.ip_fabric {
        nets.push(format!("\\textbf{{Fabric:}} \\texttt{{{fabric}}}"));
    }
    if let Some(mgmt) = &dev.ip_mgmt {
        nets.push(format!("\\textbf{{Mgmt:}} \\texttt{{{mgmt}}}"));
    }
    if !nets.is_empty() {
        lines.push(format!("{}\\\\", nets.join(" \\quad ")));
    }
    if let Some(board) = &dev.motherboard {
        lines.push(format!("\\textbf{{Board:}} {}\\\\", esc(board)));
    }
    if let Some(cpu) = &dev.cpu {
        lines.push(format!("\\textbf{{CPU:}} {}\\\\", esc(cpu)));
    }
    if let Some(ram) = dev.ram_gb {
        lines.push(format!("\\textbf{{RAM:}} {ram}\\,GB\\\\"));
    }
    if let Some(hca) = &dev.fabric_hca {
        lines.push(format!("\\textbf{{Fabric HCA:}} {}\\\\", esc(hca)));
    }
    if !dev.disks.is_empty() {
        let disks: Vec<String> = dev.disks.iter().map(|d| esc(d)).collect();
        lines.push(format!("\\textbf{{Disks:}} {}\\\\", disks.join("; ")));
    }
    let acc: String = dev
        .accelerator
        .iter()
        .map(|a| {
            format!(
                "\\item {}$\\times$ {} ({}\\,W each)\n",
                a.count,
                esc(&a.model),
                a.power_each_w
            )
        })
        .collect();
    if !acc.is_empty() {
        lines.push(format!(
            "\\textbf{{Accelerators:}}\\begin{{itemize}}[nosep,leftmargin=1.4em]{acc}\\end{{itemize}}"
        ));
    }
    let sot = dev.source_of_truth.as_deref().unwrap_or("none");
    let sot_tex = if sot == "none" {
        r"\textcolor{accent}{\textbf{not registered in the catalog}}".to_owned()
    } else {
        format!("\\texttt{{{}}}", esc(sot))
    };
    lines.push(format!("\\textbf{{Source of truth:}} {sot_tex}\\\\"));
    if let Some(notes) = &dev.notes {
        lines.push(format!("\\textcolor{{darkgray}}{{{}}}", esc(notes)));
    }
    format!("\\subsection{{{}}}\n{}", esc(&dev.name), lines.join("\n"))
}

/// Workload table plus per-host guest lists, closed by the rendered
/// `workloads-note` fragment.
pub fn workloads_section(workloads: &WorkloadsFile, note: &str) -> String {
    let wl = workloads
        .workload
        .iter()
        .map(|w| {
            format!(
                "{} & {} & {} & {} \\\\",
                esc(&w.node),
                esc(&w.name),
                esc(&w.kind),
                esc(&w.resources)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let guests: Vec<String> = workloads
        .guest
        .iter()
        .map(|g| {
            let items: String = g
                .guests
                .iter()
                .map(|x| format!("\\item {}\n", esc(x)))
                .collect();
            format!(
                "\\subsection{{Guests on {}}}\\begin{{itemize}}[nosep,leftmargin=1.4em]{items}\\end{{itemize}}",
                capitalize(&g.host)
            )
        })
        .collect();
    format!(
        "\n\\section{{Workloads \\& Placement}}\n\\begin{{tabularx}}{{\\textwidth}}{{l l l X}}\n\\toprule\n\\textbf{{Node}} & \\textbf{{Workload}} & \\textbf{{Kind}} & \\textbf{{Resources}} \\\\\n\\midrule\n{wl}\n\\bottomrule\n\\end{{tabularx}}\n{}\n\n{note}",
        guests.join("\n")
    )
}

/// First character uppercased, the rest lowercased (Python `str.capitalize`).
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Generated system-topology diagram, closed by the rendered
/// `topology-note` fragment. Everything drawn is declared data: the hub is
/// the device whose role starts with "Edge router", host nodes carry
/// board/accelerator/guest facts, down-state comes from the snapshot, the
/// fabric edge joins devices declaring a `fabric_hca`, and the mgmt rail
/// appears when `networks.mgmt` and `bmc` fields are declared.
pub fn topology_section(
    devices: &DevicesFile,
    workloads: &WorkloadsFile,
    site: &SiteFile,
    snapshot: &Snapshot,
    note: &str,
) -> String {
    let Some(hub) = devices
        .device
        .iter()
        .find(|d| d.role.starts_with("Edge router"))
        .or(devices.device.first())
    else {
        return format!("\n\\section{{System Topology}}\n{note}");
    };
    let hosts: Vec<&Device> = devices
        .device
        .iter()
        .filter(|d| d.id != hub.id && d.ip_lan.is_some())
        .collect();

    let mut tex = String::from(
        "\n\\section{System Topology}\n\\begin{center}\n\\begin{tikzpicture}[\n  node distance=7mm and 9mm,\n  box/.style={draw=secondary, thick, rounded corners=2pt, fill=lightgray!50,\n              align=center, font=\\small, inner sep=5pt},\n  host/.style={box, fill=white, draw=primary, minimum width=33mm},\n  downhost/.style={host, draw=accent, dashed},\n  lbl/.style={font=\\scriptsize\\itshape, text=darkgray},\n]\n",
    );
    writeln!(
        tex,
        "\\node[box, fill=secondary, text=white, text width=64mm] (wan) {{WAN\\\\\\scriptsize {}}};",
        esc(&site.networks.wan)
    )
    .ok();
    writeln!(
        tex,
        "\\node[host, below=of wan] ({}) {{{}}};",
        hub.id,
        node_label(hub, workloads, snapshot)
    )
    .ok();
    for (i, dev) in hosts.iter().enumerate() {
        let xshift = (i as f64 - (hosts.len() as f64 - 1.0) / 2.0) * 52.0;
        writeln!(
            tex,
            "\\node[{}, below=14mm of {}, xshift={xshift:.0}mm] ({}) {{{}}};",
            if is_down(dev, snapshot) {
                "downhost"
            } else {
                "host"
            },
            hub.id,
            dev.id,
            node_label(dev, workloads, snapshot)
        )
        .ok();
    }
    let mgmt_members: Vec<&Device> = devices
        .device
        .iter()
        .filter(|d| d.bmc.is_some() || d.ip_mgmt.is_some())
        .collect();
    if let (Some(mgmt), false) = (&site.networks.mgmt, mgmt_members.is_empty()) {
        let bmcs = mgmt_members
            .iter()
            .filter_map(|d| {
                d.bmc
                    .as_deref()
                    .or(d.ip_mgmt.as_deref())
                    .map(|b| format!("{} {}", esc(&d.name), esc(b)))
            })
            .collect::<Vec<_>>()
            .join(" · ");
        writeln!(
            tex,
            "\\node[box, right=14mm of {}] (mgmt) {{mgmt {}\\\\\\scriptsize {bmcs}}};\n\\draw[thick, secondary, dashed] ({}) -- (mgmt);",
            hub.id,
            esc(mgmt),
            hub.id
        )
        .ok();
    }
    writeln!(tex, "\\draw[thick, secondary] (wan) -- ({});", hub.id).ok();
    for (i, dev) in hosts.iter().enumerate() {
        let label = if i == 0 {
            format!(
                " node[midway, lbl, above left] {{LAN {}}}",
                esc(&site.networks.lan)
            )
        } else {
            String::new()
        };
        writeln!(
            tex,
            "\\draw[thick, primary] ({}) -- ({}){label};",
            hub.id, dev.id
        )
        .ok();
    }
    let fabric_members: Vec<&Device> = hosts
        .iter()
        .filter(|d| d.fabric_hca.is_some())
        .copied()
        .collect();
    let fabric_switch = devices
        .device
        .iter()
        .find(|d| d.role.starts_with("Fabric switch"));
    if let Some(switch) = fabric_switch {
        writeln!(
            tex,
            "\\node[host, below=34mm of {}] ({}) {{{}}};",
            hub.id,
            switch.id,
            node_label(switch, workloads, snapshot)
        )
        .ok();
        for (i, member) in fabric_members.iter().enumerate() {
            let label = if i == 0 {
                format!(
                    " node[midway, lbl, below left] {{IPoIB fabric {}}}",
                    esc(&site.networks.fabric)
                )
            } else {
                String::new()
            };
            writeln!(
                tex,
                "\\draw[very thick, dashed, success] ({}) -- ({}){label};",
                member.id, switch.id
            )
            .ok();
        }
    } else {
        for pair in fabric_members.windows(2) {
            writeln!(
                tex,
                "\\draw[very thick, dashed, success] ({}) to[bend right=18]\n  node[midway, lbl, below] {{IPoIB fabric {}}} ({});",
                pair[0].id,
                esc(&site.networks.fabric),
                pair[1].id
            )
            .ok();
        }
    }
    tex.push_str("\\end{tikzpicture}\n\\end{center}\n\n");
    tex.push_str(note);
    tex
}

fn is_down(dev: &Device, snapshot: &Snapshot) -> bool {
    snapshot
        .results
        .iter()
        .any(|r| r.id == dev.id && r.state == ProbeState::Down)
}

fn node_label(dev: &Device, workloads: &WorkloadsFile, snapshot: &Snapshot) -> String {
    let mut lines = vec![match &dev.ip_lan {
        Some(lan) => format!(
            "\\textbf{{{}}} \\texttt{{.{}}}",
            esc(&dev.name),
            lan.rsplit('.').next().unwrap_or("?")
        ),
        None => format!("\\textbf{{{}}}", esc(&dev.name)),
    }];
    if let Some(board) = dev.motherboard.as_ref().or(dev.model.as_ref()) {
        lines.push(format!("\\scriptsize {}", esc(board)));
    }
    let accel = dev
        .accelerator
        .iter()
        .map(|a| format!("{}$\\times${}", a.count, esc(short_model(&a.model))))
        .collect::<Vec<_>>()
        .join(" · ");
    if !accel.is_empty() {
        lines.push(format!("\\scriptsize {accel}"));
    }
    if let Some(placement) = workloads.guest.iter().find(|g| g.host == dev.id) {
        let vms = placement
            .guests
            .iter()
            .filter(|g| g.starts_with("VM"))
            .count();
        let cts = placement
            .guests
            .iter()
            .filter(|g| g.starts_with("CT"))
            .count();
        lines.push(format!(
            "\\scriptsize {} guests ({vms} VM, {cts} CT)",
            placement.guests.len()
        ));
    }
    if is_down(dev, snapshot) {
        lines.push("\\textcolor{accent}{\\scriptsize\\textbf{DOWN}}".to_owned());
    }
    lines.join("\\\\")
}

/// The short model token drawn in node labels: the first word mixing
/// letters and digits (`V100`), else the first word containing a digit
/// (`4090`), else the whole model string.
fn short_model(model: &str) -> &str {
    let words: Vec<&str> = model.split_whitespace().collect();
    words
        .iter()
        .find(|w| {
            w.chars().any(|c| c.is_ascii_alphabetic()) && w.chars().any(|c| c.is_ascii_digit())
        })
        .or_else(|| words.iter().find(|w| w.chars().any(|c| c.is_ascii_digit())))
        .copied()
        .unwrap_or(model)
}

/// Power table and thermal-load figures.
pub fn power_section(model: &PowerModel, site: &SiteFile, thermal: &ThermalFile) -> String {
    let tariff = site.power.tariff_usd_per_kwh;
    let hours = site.power.hours_per_month;
    let cooling = &thermal.cooling;
    let btu = model.peak_w * cooling.btu_per_watt_hour;
    let tons = btu / cooling.ton_refrigeration_btu_h;
    let body = model
        .rows
        .iter()
        .map(|r| {
            let down_mark = if r.is_up {
                ""
            } else {
                r" \textcolor{accent}{(down)}"
            };
            format!(
                "{}{down_mark} & {} & {} & {} & \\${} \\\\",
                esc(&r.name),
                fmt_sep(r.typical_w, 0),
                fmt_sep(r.peak_w, 0),
                fmt_sep(r.kwh_per_month, 0),
                fmt_sep(r.usd_per_month, 2)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let down_names: Vec<&str> = model
        .rows
        .iter()
        .filter(|r| !r.is_up)
        .map(|r| r.name.as_str())
        .collect();
    let live_label = if down_names.is_empty() {
        "Live today".to_owned()
    } else {
        format!("Live today ({} down)", down_names.join(", "))
    };
    format!(
        "\n\\section{{Power \\& Thermal}}\nTariff \\textbf{{\\${tariff:.2}/kWh}}, {hours}\\,h accounting month. Figures are modeled\nfrom component TDPs — no PDU metering exists yet, so treat them as a sizing\nmodel, not a bill.\n\n\\begin{{tabularx}}{{\\textwidth}}{{X r r r r}}\n\\toprule\n\\textbf{{Device}} & \\textbf{{Typical (W)}} & \\textbf{{Peak (W)}} & \\textbf{{kWh/mo}} & \\textbf{{USD/mo}} \\\\\n\\midrule\n{body}\n\\midrule\n\\textbf{{All-on typical}} & \\textbf{{{}}} & & {} & \\textbf{{\\${}}} \\\\\n\\textbf{{{live_label}}} & \\textbf{{{}}} & & {} & \\textbf{{\\${}}} \\\\\n\\textbf{{Peak envelope}} & & \\textbf{{{}}} & & \\\\\n\\bottomrule\n\\end{{tabularx}}\n\n\\subsection{{Thermal load}}\nPeak heat rejection {}\\,BTU/h ({tons:.2} tons refrigeration);\n{}. All-on annual energy cost\n\\textbf{{\\${}}}.\n",
        fmt_sep(model.all_on_typical_w, 0),
        fmt_sep(crate::power::kwh_per_month(model.all_on_typical_w, site), 0),
        fmt_sep(usd_per_month(model.all_on_typical_w, site), 2),
        fmt_sep(model.live_typical_w, 0),
        fmt_sep(crate::power::kwh_per_month(model.live_typical_w, site), 0),
        fmt_sep(usd_per_month(model.live_typical_w, site), 2),
        fmt_sep(model.peak_w, 0),
        fmt_sep(btu, 0),
        esc(&cooling.method),
        fmt_sep(usd_per_month(model.all_on_typical_w, site) * 12.0, 0),
    )
}

/// Capex ledger table and the energy-vs-capex ratio.
pub fn capex_section(capex: &CapexFile, model: &PowerModel, site: &SiteFile) -> String {
    let total: i64 = capex.item.iter().map(|i| i.usd).sum();
    let annual = usd_per_month(model.all_on_typical_w, site) * 12.0;
    let pct = annual / total as f64 * 100.0;
    let body = capex
        .item
        .iter()
        .map(|i| {
            format!(
                "{} & {} & \\${} & {} \\\\",
                esc(&i.device),
                esc(&i.desc),
                fmt_int_sep(i.usd),
                esc(&i.basis)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\n\\section{{Capital Expenditure}}\n\\textcolor{{accent}}{{All line items are used-market estimates}} entered to\nseed the model — replace with invoice values in \\texttt{{finance/capex.toml}}\n(\\texttt{{basis = \"invoice\"}}) as records are located.\n\n\\begin{{tabularx}}{{\\textwidth}}{{l X r l}}\n\\toprule\n\\textbf{{Device}} & \\textbf{{Item}} & \\textbf{{USD}} & \\textbf{{Basis}} \\\\\n\\midrule\n{body}\n\\midrule\n\\textbf{{Total}} & & \\textbf{{\\${}}} & \\\\\n\\bottomrule\n\\end{{tabularx}}\n\nAt the all-on typical load, annual power (\\${}) is\n{pct:.0}\\,\\% of the estimated fleet capex — energy dominates\nTCO within $\\sim$3 years.\n",
        fmt_int_sep(total),
        fmt_sep(annual, 0),
    )
}

#[cfg(test)]
mod tests;
