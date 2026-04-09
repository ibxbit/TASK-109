import { NavLink } from 'react-router-dom';
import {
  Users,
  Target,
  ClipboardList,
  GitMerge,
  BarChart2,
  FileText,
  Bell,
  Activity,
} from 'lucide-react';
import { useAuthStore } from '../../store/authStore';
import { ROLE_IDS, ROLE_LABELS, APP_NAME } from '../../utils/constants';

interface NavItem {
  to:         string;
  label:      string;
  icon:       React.ReactNode;
  allowRoles: string[];
}

const NAV_ITEMS: NavItem[] = [
  {
    to:         '/members',
    label:      'Members & Profiles',
    icon:       <Users size={18} />,
    allowRoles: [ROLE_IDS.ADMINISTRATOR, ROLE_IDS.CARE_COACH],
  },
  {
    to:         '/goals',
    label:      'Goals',
    icon:       <Target size={18} />,
    allowRoles: [ROLE_IDS.ADMINISTRATOR, ROLE_IDS.CARE_COACH, ROLE_IDS.MEMBER],
  },
  {
    to:         '/work-orders',
    label:      'Work Orders',
    icon:       <ClipboardList size={18} />,
    allowRoles: [ROLE_IDS.ADMINISTRATOR, ROLE_IDS.CARE_COACH],
  },
  {
    to:         '/workflows',
    label:      'Workflows',
    icon:       <GitMerge size={18} />,
    allowRoles: [ROLE_IDS.ADMINISTRATOR, ROLE_IDS.APPROVER],
  },
  {
    to:         '/analytics',
    label:      'Analytics',
    icon:       <BarChart2 size={18} />,
    allowRoles: [ROLE_IDS.ADMINISTRATOR, ROLE_IDS.CARE_COACH],
  },
  {
    to:         '/audit-logs',
    label:      'Audit Logs',
    icon:       <FileText size={18} />,
    allowRoles: [ROLE_IDS.ADMINISTRATOR],
  },
  {
    to:         '/notifications',
    label:      'Notifications',
    icon:       <Bell size={18} />,
    allowRoles: [
      ROLE_IDS.ADMINISTRATOR,
      ROLE_IDS.CARE_COACH,
      ROLE_IDS.APPROVER,
      ROLE_IDS.MEMBER,
    ],
  },
];

const baseLink =
  'flex items-center gap-3 px-3 py-2 rounded-md text-sm font-medium transition-colors duration-150';

const activeLink   = 'bg-brand-700 text-white';
const inactiveLink = 'text-brand-100 hover:bg-brand-700/60';

export function Sidebar() {
  const user   = useAuthStore((s) => s.user);
  const roleId = user?.role_id ?? '';

  const visibleItems = NAV_ITEMS.filter((item) =>
    item.allowRoles.includes(roleId),
  );

  return (
    <aside className="flex flex-col w-60 min-h-screen bg-brand-800 text-white shrink-0">
      {/* Logo / Brand */}
      <div className="flex items-center gap-3 px-5 py-5 border-b border-brand-700">
        <Activity size={22} className="text-brand-300" />
        <span className="text-sm font-bold leading-tight text-white">
          {APP_NAME}
        </span>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-3 py-4 space-y-1 overflow-y-auto">
        {visibleItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) =>
              `${baseLink} ${isActive ? activeLink : inactiveLink}`
            }
          >
            <span className="shrink-0">{item.icon}</span>
            {item.label}
          </NavLink>
        ))}
      </nav>

      {/* User info at bottom */}
      {user && (
        <div className="px-4 py-3 border-t border-brand-700 text-xs text-brand-300">
          <p className="font-medium text-white truncate">{user.username}</p>
          <p className="truncate">{ROLE_LABELS[user.role_id] ?? user.role_id}</p>
        </div>
      )}
    </aside>
  );
}
